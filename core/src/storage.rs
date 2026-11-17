//! SQLite storage.
//!
//! * Every migration is embedded in the binary and its SHA-256 is recorded. A mismatch aborts
//!   startup instead of silently repairing (docs/02 §6).
//! * Sensitive values go in as AES-256-GCM blobs; the audit table is append-only at the trigger
//!   level as well as in code.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::audit::{AuditEvent, AuditLog};
use crate::crypto::{
    blind_index, derive_master_key, open, random_salt, seal, sha256_hex, unwrap_key, wrap_key,
    SecretKey, SALT_LEN,
};
use crate::error::{CoreError, Result};
use crate::rules::{Rule, RuleKind, RulePayload, RuleSet, RuleSetKind, ReviewStatus, RulesEngine};
use crate::date::Date;

const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("migrations/0001_init.sql")),
    ("0002_user_key_wrap", include_str!("migrations/0002_user_key_wrap.sql")),
    ("0003_phase2", include_str!("migrations/0003_phase2.sql")),
    ("0004_recovery", include_str!("migrations/0004_recovery.sql")),
];

const SETTING_INDEX_KEY: &str = "key.index.v1";
const SETTING_AUDIT_KEY: &str = "key.audit.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub roles: Vec<String>,
    pub active: bool,
    pub failed_attempts: u32,
    pub locked_until_ms: Option<i64>,
    pub must_change_password: bool,
}

pub struct Database {
    conn: Connection,
    path: PathBuf,
    master: SecretKey,
    index_key: SecretKey,
    audit_key: SecretKey,
}

impl Database {
    /// Open (creating if needed) the office database and bring the schema up to date.
    pub fn open(path: impl AsRef<Path>, master: SecretKey) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        Self::apply_pragmas(&conn)?;
        Self::migrate(&conn)?;

        let (index_key, audit_key) = Self::load_or_create_aux_keys(&conn, &master)?;
        Ok(Database { conn, path, master, index_key, audit_key })
    }

    /// Open the office database by deriving the master key from a passphrase.
    pub fn open_with_passphrase(
        path: impl AsRef<Path>,
        passphrase: &str,
        salt: &[u8; SALT_LEN],
        params: crate::crypto::KdfParams,
    ) -> Result<Self> {
        let master = derive_master_key(passphrase, salt, params)?;
        Self::open(path, master)
    }

    fn apply_pragmas(conn: &Connection) -> Result<()> {
        // Never write decrypted content to a temp file, and keep durability for legal records.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             PRAGMA foreign_keys=ON;
             PRAGMA temp_store=MEMORY;
             PRAGMA busy_timeout=5000;
             PRAGMA secure_delete=ON;",
        )?;
        Ok(())
    }

    fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migration (
                 id TEXT PRIMARY KEY, checksum TEXT NOT NULL, applied_at_ms INTEGER NOT NULL);",
        )?;
        for (id, sql) in MIGRATIONS {
            let checksum = sha256_hex(sql.as_bytes());
            let recorded: Option<String> = conn
                .query_row(
                    "SELECT checksum FROM schema_migration WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()?;

            match recorded {
                Some(prev) if prev == checksum => continue,
                Some(prev) => {
                    return Err(CoreError::Storage(format!(
                        "migration {id} checksum mismatch (recorded {prev}, binary {checksum}); \
                         refusing to start"
                    )))
                }
                None => {
                    let tx = conn.unchecked_transaction()?;
                    tx.execute_batch(sql)?;
                    tx.execute(
                        "INSERT INTO schema_migration (id, checksum, applied_at_ms) VALUES (?1, ?2, ?3)",
                        params![id, checksum, crate::now_ms()],
                    )?;
                    tx.commit()?;
                }
            }
        }
        Ok(())
    }

    /// The blind-index and audit keys are random, generated once, and sealed under the master key.
    fn load_or_create_aux_keys(
        conn: &Connection,
        master: &SecretKey,
    ) -> Result<(SecretKey, SecretKey)> {
        let load = |name: &str| -> Result<SecretKey> {
            let stored: Option<Vec<u8>> = conn
                .query_row(
                    "SELECT value FROM app_setting WHERE key = ?1",
                    params![name],
                    |r| r.get(0),
                )
                .optional()?;
            match stored {
                Some(wrapped) => unwrap_key(master, &wrapped),
                None => {
                    let key = SecretKey::generate()?;
                    let wrapped = wrap_key(master, &key)?;
                    // Stored as a BLOB: the wrapped key is binary, not text.
                    conn.execute(
                        "INSERT INTO app_setting (key, value, updated_at_ms) VALUES (?1, ?2, ?3)",
                        params![name, wrapped, crate::now_ms()],
                    )?;
                    Ok(key)
                }
            }
        };
        let index_key = load(SETTING_INDEX_KEY)?;
        let audit_key = load(SETTING_AUDIT_KEY)?;
        Ok((index_key, audit_key))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The office master key. Deliberately not `pub`: callers outside `storage` get the operations
    /// they need (`encrypt_field`, `blind_index`, `wrap`), never the raw key.
    pub(crate) fn master_key(&self) -> &SecretKey {
        &self.master
    }

    /// Create the single office row. The name is stored encrypted; the id stays plaintext.
    pub fn insert_office(&self, id: &str, name: &str) -> Result<()> {
        let name_enc = self.encrypt_field(name, &format!("office:{id}:name"))?;
        self.conn.execute(
            "INSERT INTO office (id, name_enc, created_at_ms) VALUES (?1, ?2, ?3)",
            params![id, name_enc, crate::now_ms()],
        )?;
        Ok(())
    }

    pub fn office_name(&self, id: &str) -> Result<Option<String>> {
        let blob: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT name_enc FROM office WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?;
        match blob {
            Some(bytes) => Ok(Some(self.decrypt_field(&bytes, &format!("office:{id}:name"))?)),
            None => Ok(None),
        }
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    // ---- field-level encryption -------------------------------------------------------------

    pub fn encrypt_field(&self, plaintext: &str, aad: &str) -> Result<Vec<u8>> {
        seal(&self.master, plaintext.as_bytes(), aad.as_bytes())
    }

    pub fn decrypt_field(&self, blob: &[u8], aad: &str) -> Result<String> {
        let bytes = open(&self.master, blob, aad.as_bytes())?;
        String::from_utf8(bytes).map_err(|_| CoreError::Crypto("field is not utf-8".into()))
    }

    pub fn blind_index(&self, value: &str) -> [u8; 32] {
        blind_index(&self.index_key, value)
    }

    // ---- settings ---------------------------------------------------------------------------

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO app_setting (key, value, updated_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            params![key, value, crate::now_ms()],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    // ---- audit ------------------------------------------------------------------------------

    pub fn append_audit(
        &self,
        actor_id: &str,
        action: &str,
        subject: &str,
        detail: &str,
    ) -> Result<AuditEvent> {
        let last = self.last_audit_event()?;
        let log = AuditLog::new(SecretKey::from_bytes(*self.audit_key.as_bytes()));
        let event = log.append(
            last.as_ref(),
            crate::now_ms(),
            actor_id,
            action,
            subject,
            detail,
        )?;
        self.conn.execute(
            "INSERT INTO audit_event
             (seq, occurred_at_ms, actor_id, action, subject, detail, prev_hash, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.seq as i64,
                event.occurred_at_ms,
                event.actor_id,
                event.action,
                event.subject,
                event.detail,
                event.prev_hash.as_slice(),
                event.hash.as_slice()
            ],
        )?;
        Ok(event)
    }

    fn last_audit_event(&self) -> Result<Option<AuditEvent>> {
        self.conn
            .query_row(
                "SELECT seq, occurred_at_ms, actor_id, action, subject, detail, prev_hash, hash
                 FROM audit_event ORDER BY seq DESC LIMIT 1",
                [],
                Self::row_to_audit,
            )
            .optional()
            .map_err(Into::into)
    }

    fn row_to_audit(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditEvent> {
        let prev: Vec<u8> = row.get(6)?;
        let hash: Vec<u8> = row.get(7)?;
        let mut prev_hash = [0u8; 32];
        let mut ev_hash = [0u8; 32];
        if prev.len() == 32 && hash.len() == 32 {
            prev_hash.copy_from_slice(&prev);
            ev_hash.copy_from_slice(&hash);
        }
        Ok(AuditEvent {
            seq: row.get::<_, i64>(0)? as u64,
            occurred_at_ms: row.get(1)?,
            actor_id: row.get(2)?,
            action: row.get(3)?,
            subject: row.get(4)?,
            detail: row.get(5)?,
            prev_hash,
            hash: ev_hash,
        })
    }

    pub fn audit_events(&self) -> Result<Vec<AuditEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, occurred_at_ms, actor_id, action, subject, detail, prev_hash, hash
             FROM audit_event ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_audit)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Verify the chain from genesis to head. Called on startup and before every export.
    pub fn verify_audit(&self) -> Result<()> {
        let events = self.audit_events()?;
        let log = AuditLog::new(SecretKey::from_bytes(*self.audit_key.as_bytes()));
        log.verify(&events)
    }

    pub fn audit_head_hex(&self) -> Result<String> {
        Ok(AuditLog::head_hash_hex(&self.audit_events()?))
    }

    // ---- users ------------------------------------------------------------------------------

    pub fn create_user(
        &self,
        id: &str,
        username: &str,
        display_name: Option<&str>,
        password_hash: &str,
        roles: &[&str],
        must_change_password: bool,
    ) -> Result<()> {
        if username.trim().is_empty() {
            return Err(CoreError::Validation("username is required".into()));
        }
        if roles.is_empty() {
            return Err(CoreError::Validation("at least one role is required".into()));
        }
        let display_enc = match display_name {
            Some(name) => Some(self.encrypt_field(name, &format!("user:{id}:display_name"))?),
            None => None,
        };
        self.conn.execute(
            "INSERT INTO user (id, username, display_name_enc, password_hash, roles_json,
                               active, failed_attempts, created_at_ms, must_change_password)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, 0, ?6, ?7)",
            params![
                id,
                username,
                display_enc,
                password_hash,
                serde_json::to_string(roles).expect("roles serialise"),
                crate::now_ms(),
                must_change_password as i64
            ],
        )?;
        Ok(())
    }

    pub fn find_user(&self, username: &str) -> Result<Option<UserRecord>> {
        self.conn
            .query_row(
                "SELECT id, username, roles_json, active, failed_attempts, locked_until_ms,
                        must_change_password
                 FROM user WHERE username = ?1 COLLATE NOCASE",
                params![username],
                |row| {
                    let roles_json: String = row.get(2)?;
                    Ok(UserRecord {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        roles: serde_json::from_str(&roles_json).unwrap_or_default(),
                        active: row.get::<_, i64>(3)? != 0,
                        failed_attempts: row.get::<_, i64>(4)? as u32,
                        locked_until_ms: row.get(5)?,
                        must_change_password: row.get::<_, i64>(6)? != 0,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn password_hash_of(&self, user_id: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT password_hash FROM user WHERE id = ?1",
                params![user_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn record_failed_login(
        &self,
        user_id: &str,
        max_attempts: u32,
        lockout_ms: i64,
        now_ms: i64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE user SET failed_attempts = failed_attempts + 1,
                 locked_until_ms = CASE WHEN failed_attempts + 1 >= ?2 THEN ?3 ELSE locked_until_ms END
             WHERE id = ?1",
            params![user_id, max_attempts as i64, now_ms + lockout_ms],
        )?;
        Ok(())
    }

    pub fn record_successful_login(&self, user_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE user SET failed_attempts = 0, locked_until_ms = NULL WHERE id = ?1",
            params![user_id],
        )?;
        Ok(())
    }

    pub fn set_password_hash(&self, user_id: &str, hash: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE user SET password_hash = ?2, must_change_password = 0 WHERE id = ?1",
            params![user_id, hash],
        )?;
        Ok(())
    }

    pub fn count_users(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM user", [], |r| r.get(0))?)
    }

    // ---- rules ------------------------------------------------------------------------------

    pub fn load_rules_engine(&self) -> Result<RulesEngine> {
        let mut stmt = self.conn.prepare(
            "SELECT id, code, kind, version_no, valid_from, valid_to, source, status,
                    reviewed_by, reviewed_on
             FROM rule_set ORDER BY code, valid_from",
        )?;
        let sets = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? as u32,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut rule_sets = Vec::new();
        for (id, code, kind, version_no, from, to, source, status, reviewed_by, reviewed_on) in sets {
            let kind = match kind.as_str() {
                "fee" => RuleSetKind::Fee,
                "tax" => RuleSetKind::Tax,
                "deadline" => RuleSetKind::Deadline,
                "checklist" => RuleSetKind::Checklist,
                "succession" => RuleSetKind::Succession,
                other => return Err(CoreError::Storage(format!("unknown rule set kind {other}"))),
            };
            rule_sets.push(RuleSet {
                id,
                code,
                kind,
                version_no,
                valid_from: Date::parse_iso(&from)?,
                valid_to: to.as_deref().map(Date::parse_iso).transpose()?,
                source,
                status: ReviewStatus::from_code(&status)?,
                reviewed_by,
                reviewed_on: reviewed_on.as_deref().map(Date::parse_iso).transpose()?,
            });
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, rule_set_id, code, kind, label_key, priority, payload_json, status, applies_to
             FROM rule ORDER BY priority",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)? as i32,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut rules = Vec::new();
        for (id, rule_set_id, code, kind, label_key, priority, payload_json, status, applies_to) in rows {
            let kind = match kind.as_str() {
                "tranche" => RuleKind::Tranche,
                "flat" => RuleKind::Flat,
                "percentage" => RuleKind::Percentage,
                other => return Err(CoreError::Storage(format!("unknown rule kind {other}"))),
            };
            let payload: RulePayload = serde_json::from_str(&payload_json)
                .map_err(|e| CoreError::Storage(format!("bad rule payload: {e}")))?;
            rules.push(Rule {
                id,
                rule_set_id,
                code,
                kind,
                label_key,
                priority,
                payload,
                status: ReviewStatus::from_code(&status)?,
                applies_to,
            });
        }
        Ok(RulesEngine::new(rule_sets, rules))
    }

    pub fn insert_rule_set(&self, set: &RuleSet) -> Result<()> {
        self.conn.execute(
            "INSERT INTO rule_set (id, code, kind, version_no, valid_from, valid_to, source,
                                   status, reviewed_by, reviewed_on, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                set.id,
                set.code,
                set.kind.code(),
                set.version_no as i64,
                set.valid_from.to_iso(),
                set.valid_to.map(|d| d.to_iso()),
                set.source,
                set.status.code(),
                set.reviewed_by,
                set.reviewed_on.map(|d| d.to_iso()),
                crate::now_ms()
            ],
        )?;
        Ok(())
    }

    pub fn insert_rule(&self, rule: &Rule) -> Result<()> {
        self.conn.execute(
            "INSERT INTO rule (id, rule_set_id, code, kind, label_key, priority, payload_json,
                               status, applies_to)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                rule.id,
                rule.rule_set_id,
                rule.code,
                match rule.kind {
                    RuleKind::Tranche => "tranche",
                    RuleKind::Flat => "flat",
                    RuleKind::Percentage => "percentage",
                },
                rule.label_key,
                rule.priority as i64,
                serde_json::to_string(&rule.payload).expect("payload serialises"),
                rule.status.code(),
                rule.applies_to
            ],
        )?;
        Ok(())
    }

    // ---- calculations -----------------------------------------------------------------------

    pub fn store_calculation(
        &self,
        id: &str,
        calc: &crate::rules::Calculation,
        created_by: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO calculation (id, code, evaluated_on, rule_set_ids_json, lines_json,
                                      total_cents, state_levies_cents, notary_fees_cents,
                                      rounding, created_at_ms, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                id,
                calc.code,
                calc.evaluated_on.to_iso(),
                serde_json::to_string(&calc.rule_set_ids).expect("serialise"),
                serde_json::to_string(&calc.lines).expect("serialise"),
                calc.total_cents,
                calc.state_levies_cents,
                calc.notary_fees_cents,
                calc.rounding,
                crate::now_ms(),
                created_by
            ],
        )?;
        Ok(())
    }

    pub fn load_calculation(&self, id: &str) -> Result<Option<crate::rules::Calculation>> {
        let row = self
            .conn
            .query_row(
                "SELECT code, evaluated_on, rule_set_ids_json, lines_json, total_cents,
                        state_levies_cents, notary_fees_cents, rounding
                 FROM calculation WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)?,
                        r.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;

        let Some((code, on, set_ids, lines, total, state, fees, rounding)) = row else {
            return Ok(None);
        };
        Ok(Some(crate::rules::Calculation {
            code,
            evaluated_on: Date::parse_iso(&on)?,
            rule_set_ids: serde_json::from_str(&set_ids).map_err(|e| CoreError::Storage(e.to_string()))?,
            lines: serde_json::from_str(&lines).map_err(|e| CoreError::Storage(e.to_string()))?,
            total_cents: total,
            state_levies_cents: state,
            notary_fees_cents: fees,
            rounding,
            contains_unverified_rules: false,
        }))
    }
}

/// A random 16-byte KDF salt, stored beside the database as a small readable file.
pub fn create_salt_file(path: impl AsRef<Path>) -> Result<[u8; SALT_LEN]> {
    let salt = random_salt()?;
    std::fs::write(path, hex::encode(salt))?;
    Ok(salt)
}

pub fn read_salt_file(path: impl AsRef<Path>) -> Result<[u8; SALT_LEN]> {
    let text = std::fs::read_to_string(path)?;
    let bytes = hex::decode(text.trim()).map_err(|_| CoreError::Validation("bad salt file".into()))?;
    if bytes.len() != SALT_LEN {
        return Err(CoreError::Validation("bad salt file".into()));
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&bytes);
    Ok(salt)
}
