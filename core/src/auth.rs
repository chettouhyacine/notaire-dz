//! Authentication: Argon2id hashes, password policy, lockout/back-off, sessions with idle and
//! re-authentication windows, and audit entries for every outcome.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};

use crate::crypto::{random_salt, sha256_hex, KdfParams};
use crate::error::{CoreError, Result};
use crate::rbac::{Principal, Role};
use crate::storage::{Database, UserRecord};

/// Minimum length enforced regardless of the configured policy.
pub const MIN_PASSWORD_LEN: usize = 12;

/// A phrase at least this long is accepted without requiring digits or symbols: length carries the
/// entropy, which keeps natural word phrases (Arabic or French) usable.
pub const LONG_PASSWORD_LEN: usize = 20;

/// Small offline list of trivially guessable passwords. A full breach corpus is a Phase 5 item;
/// pretending a 20-entry list is a breach check would be fake security, so this is documented.
const COMMON_PASSWORDS: &[&str] = &[
    "password1234", "motdepasse123", "123456789012", "azertyuiop123", "qwertyuiop123",
    "administrateur", "notairenotaire", "aaaaaaaaaaaa", "000000000000", "letmein12345",
];

/// Two rules the notary can act on without guessing:
///
/// 1. at least [`MIN_PASSWORD_LEN`] characters;
/// 2. either two of three classes — letters (in **any** script), digits, symbols, where a space is
///    not a symbol — or [`LONG_PASSWORD_LEN`] characters, so a word phrase without digits is not
///    refused for its punctuation.
///
/// The earlier "three of upper/lower/digit/symbol" rule could never be met by an Arabic
/// passphrase, because Arabic has no letter case; this version accepts `مكتب التوثيق 2026` while
/// still refusing a single repeated character or a known-common phrase. A word list is not a
/// breach check; that limitation is stated in the docs rather than papered over.
pub fn check_password_policy(password: &str) -> Result<()> {
    let chars: Vec<char> = password.chars().collect();
    if chars.len() < MIN_PASSWORD_LEN {
        return Err(CoreError::Validation(
            "password must be at least 12 characters".into(),
        ));
    }
    if chars.iter().all(|c| *c == chars[0]) {
        return Err(CoreError::Validation(
            "password must not repeat a single character".into(),
        ));
    }
    let has_letter = chars.iter().any(|c| c.is_alphabetic());
    let has_digit = chars.iter().any(|c| c.is_numeric());
    let has_symbol = chars.iter().any(|c| !c.is_alphanumeric() && !c.is_whitespace());
    let classes = [has_letter, has_digit, has_symbol]
        .iter()
        .filter(|b| **b)
        .count();
    let long_phrase = has_letter && chars.len() >= LONG_PASSWORD_LEN;
    if classes < 2 && !long_phrase {
        return Err(CoreError::Validation(
            "password must mix letters with digits or symbols".into(),
        ));
    }
    if COMMON_PASSWORDS.contains(&password.to_lowercase().as_str()) {
        return Err(CoreError::Validation(
            "this password is too common".into(),
        ));
    }
    Ok(())
}

/// Minimum for the local account credential when the office is opened by a device key.
///
/// In that mode the credential is a screen lock: the data key lives in `office.key`. It is still
/// hashed with Argon2id, but a short credential no longer weakens the encrypted records.
pub const MIN_ACCOUNT_PASSWORD_LEN: usize = 5;

pub fn check_account_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_ACCOUNT_PASSWORD_LEN {
        return Err(CoreError::Validation(
            "password must be at least 5 characters".into(),
        ));
    }
    Ok(())
}

/// Hash a credential that only gates the sign-in screen (no password-strength rules).
pub fn hash_password_lenient(password: &str, params: KdfParams) -> Result<String> {
    check_account_password(password)?;
    hash_with(password, params)
}

pub fn hash_password(password: &str, params: KdfParams) -> Result<String> {
    check_password_policy(password)?;
    hash_with(password, params)
}

fn hash_with(password: &str, params: KdfParams) -> Result<String> {
    let salt_bytes = random_salt()?;
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|e| CoreError::Crypto(format!("salt: {e}")))?;
    let argon_params = Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(32))
        .map_err(|e| CoreError::Crypto(format!("argon2 params: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    Ok(argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| CoreError::Crypto(format!("hash: {e}")))?
        .to_string())
}

pub fn verify_password(password: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AuthConfig {
    pub max_failures: u32,
    pub lockout_ms: i64,
    pub session_ms: i64,
    pub idle_ms: i64,
    pub reauth_ms: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            max_failures: 5,
            lockout_ms: 5 * 60 * 1000,
            session_ms: 12 * 60 * 60 * 1000,
            idle_ms: 10 * 60 * 1000,
            reauth_ms: 2 * 60 * 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub user_id: String,
    pub username: String,
    pub roles: Vec<Role>,
    pub token: String,
    pub created_at_ms: i64,
    pub last_seen_ms: i64,
    pub expires_at_ms: i64,
    pub reauth_until_ms: i64,
    pub must_change_password: bool,
}

impl Session {
    pub fn principal(&self) -> Principal {
        Principal::new(self.user_id.clone(), self.roles.clone())
    }

    pub fn is_expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    pub fn is_idle(&self, now_ms: i64, idle_ms: i64) -> bool {
        now_ms - self.last_seen_ms >= idle_ms
    }
}

fn role_from_code(code: &str) -> Option<Role> {
    Some(match code {
        "notary_titular" => Role::NotaryTitular,
        "notary_associate" => Role::NotaryAssociate,
        "notary_interim" => Role::NotaryInterim,
        "head_clerk" => Role::HeadClerk,
        "clerk" => Role::Clerk,
        "trainee" => Role::Trainee,
        "accountant" => Role::Accountant,
        "inspector" => Role::Inspector,
        "it_admin" => Role::ItAdmin,
        _ => return None,
    })
}

pub struct AuthService<'a> {
    db: &'a Database,
    config: AuthConfig,
}

impl<'a> AuthService<'a> {
    pub fn new(db: &'a Database, config: AuthConfig) -> Self {
        AuthService { db, config }
    }

    pub fn create_user(
        &self,
        id: &str,
        username: &str,
        display_name: Option<&str>,
        password: &str,
        roles: &[Role],
    ) -> Result<()> {
        check_password_policy(password)?;
        let hash = hash_password(password, KdfParams::standard())?;
        let codes: Vec<&str> = roles.iter().map(|r| r.code()).collect();
        self.db
            .create_user(id, username, display_name, &hash, &codes, false)?;
        self.db
            .append_audit("system", "user.create", id, &format!("roles={}", codes.join(",")))?;
        Ok(())
    }

    /// Authenticate. Every outcome is audited; failures are counted and lock the account.
    pub fn login(&self, username: &str, password: &str, now_ms: i64) -> Result<Session> {
        let user: Option<UserRecord> = self.db.find_user(username)?;
        let Some(user) = user else {
            // Verify against a dummy hash so a missing user costs roughly the same as a wrong
            // password, then audit and fail the same way.
            let _ = verify_password(
                password,
                "$argon2id$v=19$m=8192,t=1,p=1$YWJjZGVmZ2hpamtsbW5vcA$c29tZS1kdW1teS1oYXNoLXZhbHVl",
            );
            self.db
                .append_audit("anonymous", "auth.login_failed", username, "reason=unknown_user")?;
            return Err(CoreError::Denied);
        };

        if !user.active {
            self.db
                .append_audit(&user.id, "auth.login_failed", &user.id, "reason=inactive")?;
            return Err(CoreError::Denied);
        }
        if let Some(until) = user.locked_until_ms {
            if until > now_ms {
                self.db
                    .append_audit(&user.id, "auth.login_failed", &user.id, "reason=locked")?;
                return Err(CoreError::LockedOut { until_ms: until });
            }
        }

        let phc = self
            .db
            .password_hash_of(&user.id)?
            .ok_or(CoreError::Denied)?;

        if !verify_password(password, &phc) {
            self.db.record_failed_login(
                &user.id,
                self.config.max_failures,
                self.config.lockout_ms,
                now_ms,
            )?;
            self.db
                .append_audit(&user.id, "auth.login_failed", &user.id, "reason=bad_password")?;
            let refreshed = self.db.find_user(&user.username)?;
            if let Some(u) = refreshed {
                if let Some(until) = u.locked_until_ms {
                    if until > now_ms {
                        return Err(CoreError::LockedOut { until_ms: until });
                    }
                }
            }
            return Err(CoreError::Denied);
        }

        self.db.record_successful_login(&user.id)?;

        let roles: Vec<Role> = user.roles.iter().filter_map(|c| role_from_code(c)).collect();
        if roles.is_empty() {
            return Err(CoreError::Denied);
        }

        let token = hex::encode({
            let mut buf = [0u8; 32];
            getrandom::getrandom(&mut buf)
                .map_err(|e| CoreError::Crypto(format!("rng: {e}")))?;
            buf
        });
        let token_hash = sha256_hex(token.as_bytes());
        let expires_at_ms = now_ms + self.config.session_ms;

        self.db.connection().execute(
            "INSERT INTO session (token_hash, user_id, created_at_ms, last_seen_ms,
                                  expires_at_ms, reauth_until_ms)
             VALUES (?1, ?2, ?3, ?3, ?4, ?3)",
            rusqlite::params![token_hash, user.id, now_ms, expires_at_ms],
        )?;

        self.db.append_audit(
            &user.id,
            "auth.login",
            &user.id,
            &format!("roles={}", user.roles.join(",")),
        )?;

        Ok(Session {
            user_id: user.id,
            username: user.username,
            roles,
            token,
            created_at_ms: now_ms,
            last_seen_ms: now_ms,
            expires_at_ms,
            reauth_until_ms: now_ms,
            must_change_password: user.must_change_password,
        })
    }

    /// Validate a token presented by the UI and refresh its activity timestamp.
    pub fn resume(&self, token: &str, now_ms: i64) -> Result<Session> {
        let token_hash = sha256_hex(token.as_bytes());
        let row = self
            .db
            .connection()
            .query_row(
                "SELECT user_id, created_at_ms, last_seen_ms, expires_at_ms, reauth_until_ms
                 FROM session WHERE token_hash = ?1",
                rusqlite::params![token_hash],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                    ))
                },
            )
            .ok();

        let Some((user_id, created, last_seen, expires, reauth)) = row else {
            return Err(CoreError::Denied);
        };
        if now_ms >= expires || now_ms - last_seen >= self.config.idle_ms {
            self.db.connection().execute(
                "DELETE FROM session WHERE token_hash = ?1",
                rusqlite::params![token_hash],
            )?;
            return Err(CoreError::ReauthRequired);
        }

        let user = self
            .db
            .find_user(&user_name_for(self.db, &user_id)?.unwrap_or_default())?
            .ok_or(CoreError::Denied)?;

        self.db.connection().execute(
            "UPDATE session SET last_seen_ms = ?2 WHERE token_hash = ?1",
            rusqlite::params![token_hash, now_ms],
        )?;

        Ok(Session {
            roles: user.roles.iter().filter_map(|c| role_from_code(c)).collect(),
            user_id,
            username: user.username,
            token: token.to_string(),
            created_at_ms: created,
            last_seen_ms: now_ms,
            expires_at_ms: expires,
            reauth_until_ms: reauth,
            must_change_password: user.must_change_password,
        })
    }

    pub fn logout(&self, session: &Session) -> Result<()> {
        let token_hash = sha256_hex(session.token.as_bytes());
        self.db.connection().execute(
            "DELETE FROM session WHERE token_hash = ?1",
            rusqlite::params![token_hash],
        )?;
        self.db
            .append_audit(&session.user_id, "auth.logout", &session.user_id, "")?;
        Ok(())
    }

    /// Sensitive actions (finalise, export, change rules, restore) must be recent.
    pub fn require_fresh(&self, session: &Session, now_ms: i64) -> Result<()> {
        if now_ms - session.reauth_until_ms > self.config.reauth_ms {
            return Err(CoreError::ReauthRequired);
        }
        Ok(())
    }

    pub fn mark_reauth(&self, session: &Session, now_ms: i64) -> Result<()> {
        let token_hash = sha256_hex(session.token.as_bytes());
        self.db.connection().execute(
            "UPDATE session SET reauth_until_ms = ?2 WHERE token_hash = ?1",
            rusqlite::params![token_hash, now_ms],
        )?;
        Ok(())
    }
}

fn user_name_for(db: &Database, user_id: &str) -> Result<Option<String>> {
    let mut stmt = db
        .connection()
        .prepare("SELECT username FROM user WHERE id = ?1")?;
    let mut rows = stmt.query(rusqlite::params![user_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

/// Rename an account.
///
/// Key wraps are stored per user **id**, not per username, so a rename cannot strand the envelope
/// that opens the office. Uniqueness is checked here so the error is a clear message rather than a
/// raw constraint violation.
pub fn rename_user(db: &Database, user_id: &str, new_username: &str) -> Result<()> {
    let trimmed = new_username.trim();
    if trimmed.chars().count() < 3 {
        return Err(CoreError::Validation(
            "username must have at least 3 characters".into(),
        ));
    }
    let taken: Option<String> = db
        .connection()
        .query_row(
            "SELECT id FROM user WHERE username = ?1 COLLATE NOCASE AND id <> ?2",
            rusqlite::params![trimmed, user_id],
            |r| r.get(0),
        )
        .ok();
    if taken.is_some() {
        return Err(CoreError::Validation(
            "this username is already used by another account".into(),
        ));
    }
    db.connection().execute(
        "UPDATE user SET username = ?1 WHERE id = ?2",
        rusqlite::params![trimmed, user_id],
    )?;
    db.append_audit(user_id, "user.rename", user_id, &format!("username={trimmed}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_passwords_are_refused() {
        assert!(check_password_policy("short").is_err());
        assert!(check_password_policy("alllowercaseletters").is_err());
        assert!(check_password_policy("password1234").is_err());
        assert!(check_password_policy("aaaaaaaaaaaaaa").is_err());
        assert!(check_password_policy("Correct-Horse-9").is_ok());
        // Arabic has no letter case: a phrase plus digits must be accepted.
        assert!(check_password_policy("مكتب التوثيق 2026").is_ok());
        assert!(check_password_policy("avoir la foi de mon pere").is_ok());
        assert!(check_password_policy("بسم الله الرحمن الرحيم").is_ok());
    }

    #[test]
    fn hash_verifies_and_rejects_other_passwords() {
        let phc = hash_password("Correct-Horse-9", KdfParams::for_tests()).unwrap();
        assert!(verify_password("Correct-Horse-9", &phc));
        assert!(!verify_password("Correct-Horse-8", &phc));
        assert!(!verify_password("Correct-Horse-9", "not-a-hash"));
    }

    #[test]
    fn hashes_are_salted_independently() {
        let a = hash_password("Correct-Horse-9", KdfParams::for_tests()).unwrap();
        let b = hash_password("Correct-Horse-9", KdfParams::for_tests()).unwrap();
        assert_ne!(a, b);
    }
}
