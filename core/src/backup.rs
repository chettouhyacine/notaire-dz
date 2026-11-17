//! Encrypted backups.
//!
//! A backup is a SQLite snapshot sealed with a key that is *independent of the office master key*
//! (so a lost passphrase does not also destroy the backups). The envelope keeps integrity metadata
//! outside the ciphertext so a verify run can report what it checked without decrypting everything
//! it can avoid.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::crypto::{
    derive_master_key, open_document, seal_document, sha256_hex, KdfParams, SecretKey, SALT_LEN,
};
use crate::error::{CoreError, Result};
use crate::storage::Database;

const MAGIC: &[u8; 8] = b"NZDZBAK1";

/// Derive the backup key from the office passphrase with a **different salt** than the live
/// database, so a stolen backup cannot be opened with the database key and vice versa.
/// Domain-separated on purpose: the label is part of the salt derivation.
pub fn derive_backup_key(
    passphrase: &str,
    salt: &[u8; SALT_LEN],
    params: KdfParams,
) -> Result<SecretKey> {
    let mut labelled = Vec::with_capacity(SALT_LEN + 9);
    labelled.extend_from_slice(salt);
    labelled.extend_from_slice(b"backup-v1");
    let digest = sha256_hex(&labelled);
    let bytes = hex::decode(&digest[..32])
        .map_err(|_| CoreError::Crypto("bad backup salt".into()))?;
    let mut backup_salt = [0u8; SALT_LEN];
    backup_salt.copy_from_slice(&bytes);
    derive_master_key(passphrase, &backup_salt, params)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupInfo {
    pub created_at_ms: i64,
    pub plaintext_sha256: String,
    pub plaintext_bytes: u64,
    pub audit_head_hex: String,
    pub schema_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Envelope {
    info: BackupInfo,
    /// SHA-256 of the sealed blob, so corruption is detected before decryption.
    blob_sha256: String,
}

fn temp_snapshot_path() -> Result<std::path::PathBuf> {
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf).map_err(|e| CoreError::Crypto(format!("rng: {e}")))?;
    let dir = std::env::temp_dir().join("notaire-backup");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("snapshot-{}.sqlite", hex::encode(buf))))
}

/// Take a consistent snapshot of the live database and seal it into `dest`.
pub fn create_backup(db: &Database, dest: impl AsRef<Path>, backup_key: &SecretKey) -> Result<BackupInfo> {
    let snapshot_path = temp_snapshot_path()?;

    {
        let mut dest_conn = Connection::open(&snapshot_path)?;
        let backup = rusqlite::backup::Backup::new(db.connection(), &mut dest_conn)
            .map_err(|e| CoreError::Storage(format!("backup init: {e}")))?;
        backup
            .run_to_completion(200, Duration::from_millis(5), None)
            .map_err(|e| CoreError::Storage(format!("backup run: {e}")))?;
    }

    let plaintext = std::fs::read(&snapshot_path)?;
    // Never leave a plaintext snapshot behind, even on the error path below.
    let _ = std::fs::remove_file(&snapshot_path);

    let info = BackupInfo {
        created_at_ms: crate::now_ms(),
        plaintext_sha256: sha256_hex(&plaintext),
        plaintext_bytes: plaintext.len() as u64,
        audit_head_hex: db.audit_head_hex()?,
        schema_hint: "phase1".to_string(),
    };

    let sealed = seal_document(backup_key, &plaintext, MAGIC)?;
    let envelope = Envelope {
        info: info.clone(),
        blob_sha256: sha256_hex(&sealed),
    };
    let meta_json = serde_json::to_vec(&envelope).map_err(|e| CoreError::Storage(e.to_string()))?;

    let dest = dest.as_ref();
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = Vec::with_capacity(MAGIC.len() + 4 + meta_json.len() + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(meta_json.len() as u32).to_le_bytes());
    out.extend_from_slice(&meta_json);
    out.extend_from_slice(&sealed);
    std::fs::write(dest, out)?;

    Ok(info)
}

fn read_envelope(path: &Path) -> Result<(Envelope, Vec<u8>)> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < MAGIC.len() + 4 || &bytes[..MAGIC.len()] != MAGIC {
        return Err(CoreError::Validation("not a NotaireDZ backup".into()));
    }
    let len_at = MAGIC.len();
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&bytes[len_at..len_at + 4]);
    let meta_len = u32::from_le_bytes(len_bytes) as usize;
    let meta_start = len_at + 4;
    let meta_end = meta_start
        .checked_add(meta_len)
        .filter(|e| *e <= bytes.len())
        .ok_or_else(|| CoreError::Validation("corrupt backup header".into()))?;
    let envelope: Envelope = serde_json::from_slice(&bytes[meta_start..meta_end])
        .map_err(|_| CoreError::Validation("corrupt backup metadata".into()))?;
    let sealed = bytes[meta_end..].to_vec();
    Ok((envelope, sealed))
}

/// Verify integrity and decryptability without writing anything.
pub fn verify_backup(path: impl AsRef<Path>, backup_key: &SecretKey) -> Result<BackupInfo> {
    let (envelope, sealed) = read_envelope(path.as_ref())?;
    if sha256_hex(&sealed) != envelope.blob_sha256 {
        return Err(CoreError::Crypto("backup blob does not match its checksum".into()));
    }
    let plaintext = open_document(backup_key, &sealed, MAGIC)?;
    if sha256_hex(&plaintext) != envelope.info.plaintext_sha256 {
        return Err(CoreError::Crypto("restored content does not match its checksum".into()));
    }
    Ok(envelope.info)
}

/// Restore into `target`. The caller decides when to swap the live database; nothing is overwritten
/// silently here.
pub fn restore_backup(
    path: impl AsRef<Path>,
    backup_key: &SecretKey,
    target: impl AsRef<Path>,
) -> Result<BackupInfo> {
    let (envelope, sealed) = read_envelope(path.as_ref())?;
    if sha256_hex(&sealed) != envelope.blob_sha256 {
        return Err(CoreError::Crypto("backup blob does not match its checksum".into()));
    }
    let plaintext = open_document(backup_key, &sealed, MAGIC)?;
    if sha256_hex(&plaintext) != envelope.info.plaintext_sha256 {
        return Err(CoreError::Crypto("restored content does not match its checksum".into()));
    }
    let target = target.as_ref();
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(target, &plaintext)?;
    Ok(envelope.info)
}

/// `true` when the file is a NotaireDZ backup (cheap header check, no key needed).
pub fn is_backup_file(path: impl AsRef<Path>) -> bool {
    let mut buf = [0u8; 8];
    match std::fs::File::open(path.as_ref()) {
        Ok(mut f) => f.read_exact(&mut buf).is_ok() && &buf == MAGIC,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretKey;
    use crate::rbac::Role;
    use crate::storage::Database;

    fn seeded_db(dir: &Path) -> Database {
        let master = SecretKey::from_bytes([1u8; 32]);
        let db = Database::open(dir.join("office.sqlite"), master).unwrap();
        let hash = crate::auth::hash_password("Correct-Horse-9", crate::crypto::KdfParams::for_tests())
            .unwrap();
        db.create_user("u1", "notaire", Some("Maître Démo"), &hash, &[Role::NotaryTitular.code()], false)
            .unwrap();
        db.append_audit("u1", "office.init", "office-1", "").unwrap();
        db.append_audit("u1", "case.read", "case-1", "reason=consultation").unwrap();
        db
    }

    #[test]
    fn backup_roundtrip_preserves_rows_and_head_hash() {
        let dir = tempfile::tempdir().unwrap();
        let db = seeded_db(dir.path());
        let key = SecretKey::from_bytes([9u8; 32]);
        let backup_path = dir.path().join("backups/nightly.nzdzbak");

        let info = create_backup(&db, &backup_path, &key).unwrap();
        assert!(is_backup_file(&backup_path));
        assert_eq!(info.audit_head_hex, db.audit_head_hex().unwrap());

        let verified = verify_backup(&backup_path, &key).unwrap();
        assert_eq!(verified, info);

        let restore_path = dir.path().join("restored/office.sqlite");
        restore_backup(&backup_path, &key, &restore_path).unwrap();

        let restored = Database::open(&restore_path, SecretKey::from_bytes([1u8; 32])).unwrap();
        assert_eq!(restored.count_users().unwrap(), 1);
        assert_eq!(restored.audit_head_hex().unwrap(), db.audit_head_hex().unwrap());
        restored.verify_audit().unwrap();
    }

    #[test]
    fn backup_key_is_domain_separated_from_the_database_key() {
        let salt = [5u8; SALT_LEN];
        let params = KdfParams::for_tests();
        let db_key = derive_master_key("Correct-Horse-9", &salt, params).unwrap();
        let bk = derive_backup_key("Correct-Horse-9", &salt, params).unwrap();
        assert_ne!(db_key.as_bytes(), bk.as_bytes());
        // Deterministic for the same inputs, so any backup can be reopened later.
        assert_eq!(bk.as_bytes(), derive_backup_key("Correct-Horse-9", &salt, params).unwrap().as_bytes());
        // A different passphrase does not reproduce it.
        assert_ne!(bk.as_bytes(), derive_backup_key("Correct-Horse-8", &salt, params).unwrap().as_bytes());
    }

    #[test]
    fn wrong_key_does_not_decrypt() {
        let dir = tempfile::tempdir().unwrap();
        let db = seeded_db(dir.path());
        let key = SecretKey::from_bytes([9u8; 32]);
        let path = dir.path().join("b.bak");
        create_backup(&db, &path, &key).unwrap();
        assert!(verify_backup(&path, &SecretKey::from_bytes([8u8; 32])).is_err());
    }

    #[test]
    fn corrupted_backup_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = seeded_db(dir.path());
        let key = SecretKey::from_bytes([9u8; 32]);
        let path = dir.path().join("b.bak");
        create_backup(&db, &path, &key).unwrap();

        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 5;
        bytes[last] ^= 0xff;
        std::fs::write(&path, bytes).unwrap();
        assert!(verify_backup(&path, &key).is_err());
    }

    #[test]
    fn plaintext_database_is_not_present_in_the_backup() {
        let dir = tempfile::tempdir().unwrap();
        let db = seeded_db(dir.path());
        let key = SecretKey::from_bytes([9u8; 32]);
        let path = dir.path().join("b.bak");
        create_backup(&db, &path, &key).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        // SQLite header magic must not appear in a sealed backup.
        assert!(!bytes.windows(16).any(|w| w == b"SQLite format 3\0"));
    }
}
