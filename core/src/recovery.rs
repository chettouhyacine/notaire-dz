//! Recovery codes and passphrase reset.
//!
//! The office master key is derived from the passphrase, so a forgotten passphrase would otherwise
//! mean permanently unreadable data. Setup therefore issues a **recovery code**: 120 bits of
//! randomness printed once to the notary. Only its effect is stored — the master key sealed under a
//! key derived from the code — so the code itself is never written to disk.
//!
//! Resetting a passphrase unwraps the master key with the code, re-wraps it under the new
//! passphrase and re-hashes the user's password. The master key never changes, so every record,
//! document key and audit entry stays valid. A new recovery code is issued and the old escrow is
//! replaced, because a code that has been typed into a form should not stay usable forever.
//!
//! What this is not: there is no server, no reset link and no recovery without either the
//! passphrase or the code. That is the point of encrypting at rest.

use std::path::Path;

use hmac::{Hmac, Mac};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::Sha256;

use crate::auth::{check_password_policy, hash_password};
use crate::crypto::{derive_master_key, open, random_salt, seal, KdfParams, SecretKey, SALT_LEN};
use crate::error::{CoreError, Result};
use crate::storage::Database;

type HmacSha256 = Hmac<Sha256>;

/// Crockford base32: no I, L, O or U, so codes cannot be misread for 1/0.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const GROUP_LEN: usize = 4;
pub const GROUPS: usize = 6;
/// 24 symbols × 5 bits = 120 bits, written as six groups of four.
pub const CODE_LEN: usize = GROUPS * GROUP_LEN;

const ESCROW_AAD: &[u8] = b"notaire:recovery:escrow:v1";

/// A connection for looking at the office file before any key exists.
///
/// Read-only first, because nothing here should write. A WAL database without a writable `-shm`
/// cannot be opened read-only, so a normal handle is the fallback — no row is created by reading,
/// and an existing file is never replaced.
fn peek_connection(path: &Path) -> Option<Connection> {
    if !path.exists() {
        return None;
    }
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
    .or_else(|| Connection::open(path).ok())
}
const MASTER_WRAP_AAD: &[u8] = b"notaire:master-wrap:v1";

/// A fresh code, e.g. `7K3Q-2M9X-B4TD-8JHV-6YPA-3C5R`.
pub fn generate_code() -> Result<String> {
    let mut bytes = [0u8; CODE_LEN];
    getrandom::getrandom(&mut bytes).map_err(|e| CoreError::Crypto(format!("rng: {e}")))?;
    let mut out = String::with_capacity(CODE_LEN + GROUPS - 1);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && i % GROUP_LEN == 0 {
            out.push('-');
        }
        // 256 is a multiple of 32, so the reduction is unbiased.
        out.push(ALPHABET[*byte as usize % 32] as char);
    }
    Ok(out)
}

/// Accept what a human types: lower case, missing dashes, `O`/`I`/`L` for `0`/`1`.
pub fn normalize_code(input: &str) -> Result<String> {
    let mut out = String::with_capacity(CODE_LEN);
    for ch in input.chars() {
        let upper = ch.to_ascii_uppercase();
        let mapped = match upper {
            'O' => '0',
            'I' | 'L' => '1',
            'U' => 'V',
            ' ' | '-' | '\u{2010}'..='\u{2015}' => continue,
            other => other,
        };
        if !ALPHABET.contains(&(mapped as u8)) {
            return Err(CoreError::Validation(
                "recovery code contains a character that is not part of a code".into(),
            ));
        }
        out.push(mapped);
    }
    if out.len() != CODE_LEN {
        return Err(CoreError::Validation(
            "recovery code must have 24 symbols".into(),
        ));
    }
    Ok(out)
}

/// Derive the wrapping key from the code. The code is already 120 random bits, so a fast PRF is
/// appropriate here — Argon2 is for human-chosen secrets.
fn code_key(normalized: &str) -> Result<SecretKey> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(b"notaire:recovery:v1")
        .expect("HMAC accepts any key length");
    mac.update(normalized.as_bytes());
    let tag = mac.finalize().into_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&tag);
    Ok(SecretKey::from_bytes(key))
}

pub fn seal_master(code: &str, master: &SecretKey) -> Result<Vec<u8>> {
    let key = code_key(&normalize_code(code)?)?;
    seal(&key, master.as_bytes(), ESCROW_AAD)
}

pub fn open_master(code: &str, blob: &[u8]) -> Result<SecretKey> {
    let key = code_key(&normalize_code(code)?)?;
    let bytes = open(&key, blob, ESCROW_AAD)?;
    if bytes.len() != 32 {
        return Err(CoreError::Crypto("bad master key length".into()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(SecretKey::from_bytes(out))
}

// ---- passphrase wrap ---------------------------------------------------------------------------

/// Seal the master key under a key derived from a passphrase and its own salt.
pub fn wrap_master_for_passphrase(
    passphrase: &str,
    salt: &[u8; SALT_LEN],
    params: KdfParams,
    master: &SecretKey,
) -> Result<Vec<u8>> {
    let key = derive_master_key(passphrase, salt, params)?;
    seal(&key, master.as_bytes(), MASTER_WRAP_AAD)
}

pub fn unwrap_master_with_passphrase(
    passphrase: &str,
    salt: &[u8; SALT_LEN],
    params: KdfParams,
    blob: &[u8],
) -> Result<SecretKey> {
    let key = derive_master_key(passphrase, salt, params)?;
    let bytes = open(&key, blob, MASTER_WRAP_AAD)?;
    if bytes.len() != 32 {
        return Err(CoreError::Crypto("bad master key length".into()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(SecretKey::from_bytes(out))
}

// ---- storage -----------------------------------------------------------------------------------

pub struct PassphraseWrap {
    pub salt: [u8; SALT_LEN],
    pub params: KdfParams,
    pub wrapped: Vec<u8>,
}

pub fn store_passphrase_wrap(
    conn: &Connection,
    user_id: &str,
    salt: &[u8; SALT_LEN],
    wrapped: &[u8],
    params: KdfParams,
) -> Result<()> {
    conn.execute(
        "INSERT INTO user_key_wrap
             (user_id, salt_hex, wrapped_key, kdf_m_cost, kdf_t_cost, kdf_p_cost, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(user_id) DO UPDATE SET
             salt_hex = excluded.salt_hex,
             wrapped_key = excluded.wrapped_key,
             kdf_m_cost = excluded.kdf_m_cost,
             kdf_t_cost = excluded.kdf_t_cost,
             kdf_p_cost = excluded.kdf_p_cost,
             created_at_ms = excluded.created_at_ms",
        params![
            user_id,
            hex::encode(salt),
            wrapped,
            params.m_cost_kib,
            params.t_cost,
            params.p_cost,
            crate::now_ms()
        ],
    )?;
    Ok(())
}

pub fn read_passphrase_wrap(conn: &Connection, username: &str) -> Result<Option<PassphraseWrap>> {
    let row: Option<(String, Vec<u8>, u32, u32, u32)> = conn
        .query_row(
            "SELECT w.salt_hex, w.wrapped_key, w.kdf_m_cost, w.kdf_t_cost, w.kdf_p_cost
             FROM user_key_wrap w JOIN user u ON u.id = w.user_id
             WHERE u.username = ?1 COLLATE NOCASE",
            params![username],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((salt_hex, wrapped, m, t, p)) = row else {
        return Ok(None);
    };
    let raw = hex::decode(salt_hex)
        .map_err(|_| CoreError::Storage("stored salt is not hex".into()))?;
    if raw.len() != SALT_LEN {
        return Err(CoreError::Storage("stored salt has the wrong length".into()));
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&raw);
    Ok(Some(PassphraseWrap {
        salt,
        params: KdfParams { m_cost_kib: m, t_cost: t, p_cost: p },
        wrapped,
    }))
}

/// Read a user's wrap straight from the office file, before any key is available.
///
/// An office created before wraps were written has no such row, and reports `Ok(None)` so the
/// caller can fall back to the original derive-from-passphrase path.
pub fn read_wrap_file(path: &Path, username: &str) -> Result<Option<PassphraseWrap>> {
    let Some(conn) = peek_connection(path) else {
        return Ok(None);
    };
    match read_passphrase_wrap(&conn, username) {
        Ok(found) => Ok(found),
        Err(CoreError::Storage(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn store_escrow(conn: &Connection, wrapped_master: &[u8]) -> Result<()> {
    conn.execute(
        "INSERT INTO recovery_escrow (id, wrapped_master, created_at_ms) VALUES ('office', ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET
             wrapped_master = excluded.wrapped_master,
             created_at_ms = excluded.created_at_ms",
        params![wrapped_master, crate::now_ms()],
    )?;
    Ok(())
}

pub fn read_escrow(conn: &Connection) -> Result<Option<Vec<u8>>> {
    Ok(conn
        .query_row(
            "SELECT wrapped_master FROM recovery_escrow WHERE id = 'office'",
            [],
            |r| r.get(0),
        )
        .optional()?)
}

/// Read the escrow straight from the office file, before any key is available.
///
/// The database file is ordinary SQLite (field values are encrypted, and the recovery escrow is
/// itself a sealed blob), so this needs no master key. A file without the table — an office created
/// before recovery codes existed — reports `Ok(None)` rather than failing.
pub fn read_escrow_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let Some(conn) = peek_connection(path) else {
        return Ok(None);
    };
    match read_escrow(&conn) {
        Ok(found) => Ok(found),
        Err(CoreError::Storage(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Set a new passphrase for a user after the master key has been recovered.
///
/// The audit entry is written here so a reset can never happen without a trace, whichever path the
/// caller took to unlock the office.
pub fn reset_user_passphrase(db: &Database, username: &str, new_password: &str) -> Result<()> {
    check_password_policy(new_password)?;
    let user = db.find_user(username)?.ok_or(CoreError::Denied)?;
    let hash = hash_password(new_password, KdfParams::standard())?;
    db.connection().execute(
        "UPDATE user SET password_hash = ?1, failed_attempts = 0, locked_until_ms = NULL,
                         must_change_password = 0
         WHERE id = ?2",
        params![hash, user.id],
    )?;
    db.append_audit(&user.id, "auth.passphrase_reset", &user.id, "via=recovery_code")?;
    Ok(())
}

/// Change the password of a signed-in user.
///
/// The current password is verified against the stored hash, not against a session: a stolen
/// unlocked window must not be enough to lock the owner out. The passphrase wrap is replaced so the
/// office still opens with the new password, and the master key is untouched.
pub fn change_password(
    db: &Database,
    user_id: &str,
    current: &str,
    new_password: &str,
) -> Result<()> {
    let phc = db.password_hash_of(user_id)?.ok_or(CoreError::Denied)?;
    if !crate::auth::verify_password(current, &phc) {
        return Err(CoreError::Validation(
            "current password is incorrect".into(),
        ));
    }
    check_password_policy(new_password)?;
    let hash = hash_password(new_password, KdfParams::standard())?;
    db.connection().execute(
        "UPDATE user SET password_hash = ?1, must_change_password = 0 WHERE id = ?2",
        params![hash, user_id],
    )?;

    let params = KdfParams::standard();
    let salt = random_salt()?;
    let wrapped = wrap_master_for_passphrase(new_password, &salt, params, db.master_key())?;
    store_passphrase_wrap(db.connection(), user_id, &salt, &wrapped, params)?;

    db.append_audit(user_id, "user.password_change", user_id, "via=account")?;
    Ok(())
}

/// Change the account credential when the office is opened by a device key.
///
/// Nothing is re-wrapped: the data key is not derived from this credential, so any credential of at
/// least [`crate::auth::MIN_ACCOUNT_PASSWORD_LEN`] characters is fine. The current one is still
/// required so an unattended window cannot lock the owner out.
pub fn change_credential_only(
    db: &Database,
    user_id: &str,
    current: &str,
    new_password: &str,
) -> Result<()> {
    let phc = db.password_hash_of(user_id)?.ok_or(CoreError::Denied)?;
    if !crate::auth::verify_password(current, &phc) {
        return Err(CoreError::Validation(
            "current password is incorrect".into(),
        ));
    }
    let hash = crate::auth::hash_password_lenient(new_password, KdfParams::standard())?;
    db.connection().execute(
        "UPDATE user SET password_hash = ?1, must_change_password = 0 WHERE id = ?2",
        params![hash, user_id],
    )?;
    db.append_audit(user_id, "user.password_change", user_id, "via=account")?;
    Ok(())
}

/// Replace the escrow after a reset and return the new code for the notary to write down.
pub fn reissue_escrow(db: &Database, master: &SecretKey) -> Result<String> {
    let code = generate_code()?;
    store_escrow(db.connection(), &seal_master(&code, master)?)?;
    db.append_audit("system", "recovery.reissue", "office", "reason=passphrase_reset")?;
    Ok(code)
}

/// A new salt for the passphrase wrap; independent of the office salt file.
pub fn new_wrap_salt() -> Result<[u8; SALT_LEN]> {
    random_salt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthConfig, AuthService};
    use crate::rbac::Role;

    #[test]
    fn codes_round_trip_and_can_be_typed_loosely() {
        let code = generate_code().unwrap();
        assert_eq!(code.len(), CODE_LEN + GROUPS - 1);
        assert_eq!(normalize_code(&code).unwrap().len(), CODE_LEN);

        let messy = format!(" {} ", code.to_lowercase().replace('-', " "));
        assert_eq!(normalize_code(&messy).unwrap(), normalize_code(&code).unwrap());

        assert!(normalize_code("too-short").is_err());
        assert!(normalize_code("AAAA-AAAA-AAAA-AAAA-AAAA-AAA!").is_err());
    }

    #[test]
    fn only_the_right_code_unlocks_the_master_key() {
        let master = SecretKey::generate().unwrap();
        let code = generate_code().unwrap();
        let blob = seal_master(&code, &master).unwrap();
        assert_eq!(
            open_master(&code, &blob).unwrap().as_bytes(),
            master.as_bytes()
        );

        let other = generate_code().unwrap();
        assert!(open_master(&other, &blob).is_err());
        // Tampering is detected rather than silently producing a wrong key.
        let mut broken = blob.clone();
        *broken.last_mut().unwrap() ^= 0x01;
        assert!(open_master(&code, &broken).is_err());
    }

    #[test]
    fn device_key_office_accepts_a_short_credential() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("office.sqlite");
        let params = KdfParams::for_tests();
        // The device key is a real secret (written next to the database); `admin` is only the lock.
        let key = "0f1e2d3c4b5a69788796a5b4c3d2e1f09x";
        std::fs::write(dir.path().join("office.key"), key).unwrap();

        let salt = random_salt().unwrap();
        let master = derive_master_key(key, &salt, params).unwrap();
        let db = Database::open(&path, master).unwrap();
        db.insert_office("office-1", "مكتب التوثيق").unwrap();
        let hash = crate::auth::hash_password_lenient("admin", params).unwrap();
        db.create_user("u1", "admin", Some("الموثق"), &hash, &["notary_titular"], false)
            .unwrap();

        let auth = crate::auth::AuthService::new(&db, crate::auth::AuthConfig::default());
        assert!(auth.login("admin", "admin", crate::now_ms()).is_ok());
        assert!(auth.login("admin", "wrong", crate::now_ms()).is_err());

        // The credential can be changed without touching the key: the office still opens.
        change_credential_only(&db, "u1", "admin", "admin2026").unwrap();
        let auth = crate::auth::AuthService::new(&db, crate::auth::AuthConfig::default());
        assert!(auth.login("admin", "admin2026", crate::now_ms()).is_ok());
        assert!(db.office_name("office-1").unwrap().unwrap() == "مكتب التوثيق");
        db.verify_audit().unwrap();
    }

    #[test]
    fn forgotten_passphrase_can_be_reset_without_losing_the_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("office.sqlite");
        let params = KdfParams::for_tests();
        let salt = random_salt().unwrap();
        let master = derive_master_key("Correct-Horse-9", &salt, params).unwrap();

        let code = {
            let db = Database::open(&path, master.clone()).unwrap();
            let auth = AuthService::new(&db, AuthConfig::default());
            auth.create_user("u1", "notaire", Some("Notaire"), "Correct-Horse-9", &[Role::NotaryTitular])
                .unwrap();
            db.insert_office("office-1", "Étude Test").unwrap();
            let code = generate_code().unwrap();
            store_escrow(db.connection(), &seal_master(&code, &master).unwrap()).unwrap();
            let wrapped =
                wrap_master_for_passphrase("Correct-Horse-9", &salt, params, &master).unwrap();
            store_passphrase_wrap(db.connection(), "u1", &salt, &wrapped, params).unwrap();
            code
        };

        // The passphrase is forgotten; the code comes from the notary's paper copy.
        let blob = read_escrow_file(&path).unwrap().unwrap();
        let recovered = open_master(&code, &blob).unwrap();
        assert_eq!(recovered.as_bytes(), master.as_bytes());

        let db = Database::open(&path, recovered.clone()).unwrap();
        reset_user_passphrase(&db, "notaire", "nouveau-mot-de-passe-2026").unwrap();
        let new_salt = random_salt().unwrap();
        let wrapped =
            wrap_master_for_passphrase("nouveau-mot-de-passe-2026", &new_salt, params, &recovered)
                .unwrap();
        store_passphrase_wrap(db.connection(), "u1", &new_salt, &wrapped, params).unwrap();
        let new_code = reissue_escrow(&db, &recovered).unwrap();
        assert_ne!(new_code, code);

        // The old passphrase no longer opens the envelope; the new one yields the same master key.
        let wrap = read_passphrase_wrap(db.connection(), "notaire").unwrap().unwrap();
        assert!(unwrap_master_with_passphrase(
            "Correct-Horse-9",
            &wrap.salt,
            wrap.params,
            &wrap.wrapped
        )
        .is_err());
        let via_new = unwrap_master_with_passphrase(
            "nouveau-mot-de-passe-2026",
            &wrap.salt,
            wrap.params,
            &wrap.wrapped,
        )
        .unwrap();
        assert_eq!(via_new.as_bytes(), master.as_bytes());

        // The office still opens, the name still decrypts and the new passphrase signs in.
        assert_eq!(db.office_name("office-1").unwrap().unwrap(), "Étude Test");
        let auth = AuthService::new(&db, AuthConfig::default());
        assert!(auth
            .login("notaire", "nouveau-mot-de-passe-2026", crate::now_ms())
            .is_ok());
        db.verify_audit().unwrap();

        // The code that was used for the reset no longer opens the office.
        let replaced = read_escrow_file(&path).unwrap().unwrap();
        assert!(open_master(&code, &replaced).is_err());
        assert!(open_master(&new_code, &replaced).is_ok());
    }
}
