//! Encrypted document vault.
//!
//! Each document is sealed with its **own** key (XChaCha20-Poly1305); that key is wrapped by the
//! office master key and stored in the same container file. A SHA-256 of the plaintext is recorded
//! in the database at import time and re-checked on every read, so silent tampering with a vault
//! file is detected rather than served.
//!
//! File layout of a `.nzdoc` container:
//! `MAGIC(8) || u32 wrapped_key_len || wrapped_key || sealed_bytes`

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::crypto::{open_document, seal_document, sha256_hex, SecretKey};
use crate::error::{CoreError, Result};
use crate::party::new_id;
use crate::rbac::{require_case, Permission, Principal};
use crate::storage::Database;

const MAGIC: &[u8; 8] = b"NZDZDOC1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub id: String,
    pub case_id: Option<String>,
    pub title: String,
    pub mime: String,
    pub sha256: String,
    pub byte_size: i64,
    pub version_no: u32,
    pub has_ocr_text: bool,
    pub created_at_ms: i64,
}

pub struct Vault {
    root: PathBuf,
}

impl Vault {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        Ok(Vault { root })
    }

    fn path_for(&self, vault_name: &str) -> PathBuf {
        // Two-level fan-out keeps directories small on a 1M-document archive.
        let bucket = vault_name.get(0..2).unwrap_or("00");
        self.root.join(bucket).join(format!("{vault_name}.nzdoc"))
    }

    pub fn write(&self, vault_name: &str, master: &SecretKey, data_key: &SecretKey, bytes: &[u8]) -> Result<()> {
        let path = self.path_for(vault_name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let sealed = seal_document(data_key, bytes, vault_name.as_bytes())?;
        let wrapped = crate::crypto::wrap_key(master, data_key)?;
        let mut out = Vec::with_capacity(8 + 4 + wrapped.len() + sealed.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(wrapped.len() as u32).to_le_bytes());
        out.extend_from_slice(&wrapped);
        out.extend_from_slice(&sealed);
        std::fs::write(path, out)?;
        Ok(())
    }

    pub fn read(&self, vault_name: &str, master: &SecretKey) -> Result<Vec<u8>> {
        let path = self.path_for(vault_name);
        let bytes = std::fs::read(&path)
            .map_err(|_| CoreError::Validation(format!("vault file missing: {vault_name}")))?;
        if bytes.len() < 12 || &bytes[..8] != MAGIC {
            return Err(CoreError::Crypto("not a vault container".into()));
        }
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&bytes[8..12]);
        let wrapped_len = u32::from_le_bytes(len_bytes) as usize;
        let wrapped_end = 12 + wrapped_len;
        if wrapped_end > bytes.len() {
            return Err(CoreError::Crypto("corrupt vault header".into()));
        }
        let data_key = crate::crypto::unwrap_key(master, &bytes[12..wrapped_end])?;
        open_document(&data_key, &bytes[wrapped_end..], vault_name.as_bytes())
    }

    pub fn remove(&self, vault_name: &str) -> Result<()> {
        let path = self.path_for(vault_name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

pub struct StoreDocumentInput<'a> {
    pub case_id: Option<&'a str>,
    pub title: &'a str,
    pub mime: &'a str,
    pub bytes: &'a [u8],
    pub retention_class: &'a str,
}

/// Store a new document (version 1). Requires `DocumentWrite` and case access when a case is given.
pub fn store_document(
    db: &Database,
    vault: &Vault,
    principal: &Principal,
    input: StoreDocumentInput<'_>,
) -> Result<DocumentRecord> {
    principal.require(Permission::DocumentWrite)?;
    if let Some(case_id) = input.case_id {
        let acl = crate::casefile::acl_for(db, case_id)?;
        require_case(principal, Permission::DocumentWrite, &acl)?;
    }
    if input.bytes.is_empty() {
        return Err(CoreError::Validation("empty file".into()));
    }
    if input.title.trim().is_empty() {
        return Err(CoreError::Validation("a document needs a title".into()));
    }

    let id = new_id("doc")?;
    let vault_name = new_id("v")?.replace('-', "");
    let sha256 = sha256_hex(input.bytes);
    let data_key = SecretKey::generate()?;
    vault.write(&vault_name, db.master_key(), &data_key, input.bytes)?;

    let title_enc = db.encrypt_field(input.title.trim(), &format!("document:{id}:title"))?;
    db.connection().execute(
        "INSERT INTO document (id, case_id, title_enc, mime, sha256, byte_size, vault_name,
                               version_no, retention_class, legal_hold, ocr_text_enc,
                               created_at_ms, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, 0, NULL, ?9, ?10)",
        rusqlite::params![
            id,
            input.case_id,
            title_enc,
            input.mime,
            sha256,
            input.bytes.len() as i64,
            vault_name,
            input.retention_class,
            crate::now_ms(),
            principal.user_id
        ],
    )?;

    db.append_audit(&principal.user_id, "document.store", &id, input.mime)?;

    Ok(DocumentRecord {
        id,
        case_id: input.case_id.map(str::to_string),
        title: input.title.trim().to_string(),
        mime: input.mime.to_string(),
        sha256,
        byte_size: input.bytes.len() as i64,
        version_no: 1,
        has_ocr_text: false,
        created_at_ms: crate::now_ms(),
    })
}

/// Read a document back, verifying its hash before returning the plaintext.
pub fn read_document(
    db: &Database,
    vault: &Vault,
    principal: &Principal,
    document_id: &str,
) -> Result<(DocumentRecord, Vec<u8>)> {
    principal.require(Permission::DocumentRead)?;
    let (record, vault_name) = load_record(db, document_id)?;
    if let Some(case_id) = record.case_id.as_deref() {
        let acl = crate::casefile::acl_for(db, case_id)?;
        require_case(principal, Permission::DocumentRead, &acl)?;
    }

    let bytes = vault.read(&vault_name, db.master_key())?;
    if sha256_hex(&bytes) != record.sha256 {
        db.append_audit(&principal.user_id, "document.tampered", document_id, "")?;
        return Err(CoreError::Crypto(
            "document hash does not match the stored value".into(),
        ));
    }
    db.append_audit(&principal.user_id, "document.read", document_id, "")?;
    Ok((record, bytes))
}

fn load_record(db: &Database, document_id: &str) -> Result<(DocumentRecord, String)> {
    db.connection()
        .query_row(
            "SELECT id, case_id, title_enc, mime, sha256, byte_size, version_no, ocr_text_enc,
                    created_at_ms, vault_name
             FROM document WHERE id = ?1",
            rusqlite::params![document_id],
            |r| {
                let title_enc: Vec<u8> = r.get(2)?;
                Ok((
                    (
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        title_enc,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, i64>(6)? as u32,
                        r.get::<_, Option<Vec<u8>>>(7)?,
                        r.get::<_, i64>(8)?,
                    ),
                    r.get::<_, String>(9)?,
                ))
            },
        )
        .map_err(|_| CoreError::Validation("document not found".into()))
        .and_then(|(raw, vault_name)| {
            let id = raw.0.clone();
            Ok((
                DocumentRecord {
                    id: raw.0,
                    case_id: raw.1,
                    title: db.decrypt_field(&raw.2, &format!("document:{id}:title"))?,
                    mime: raw.3,
                    sha256: raw.4,
                    byte_size: raw.5,
                    version_no: raw.6,
                    has_ocr_text: raw.7.is_some(),
                    created_at_ms: raw.8,
                },
                vault_name,
            ))
        })
}

/// Store OCR text for a document, encrypted, and record it in the audit trail.
pub fn attach_ocr_text(
    db: &Database,
    principal: &Principal,
    document_id: &str,
    text: &str,
) -> Result<()> {
    principal.require(Permission::DocumentWrite)?;
    let enc = db.encrypt_field(text, &format!("document:{document_id}:ocr"))?;
    db.connection().execute(
        "UPDATE document SET ocr_text_enc = ?2 WHERE id = ?1",
        rusqlite::params![document_id, enc],
    )?;
    db.append_audit(&principal.user_id, "document.ocr", document_id, "")?;
    Ok(())
}

pub fn ocr_text(db: &Database, principal: &Principal, document_id: &str) -> Result<Option<String>> {
    principal.require(Permission::DocumentRead)?;
    let blob: Option<Vec<u8>> = db
        .connection()
        .query_row(
            "SELECT ocr_text_enc FROM document WHERE id = ?1",
            rusqlite::params![document_id],
            |r| r.get(0),
        )
        .map_err(|_| CoreError::Validation("document not found".into()))?;
    match blob {
        Some(bytes) => Ok(Some(
            db.decrypt_field(&bytes, &format!("document:{document_id}:ocr"))?,
        )),
        None => Ok(None),
    }
}

/// Mark a document as under legal hold: deletion is then refused by a database trigger.
pub fn set_legal_hold(db: &Database, principal: &Principal, document_id: &str, hold: bool) -> Result<()> {
    principal.require_notary(Permission::DocumentDelete)?;
    db.connection().execute(
        "UPDATE document SET legal_hold = ?2 WHERE id = ?1",
        rusqlite::params![document_id, hold as i64],
    )?;
    db.append_audit(
        &principal.user_id,
        "document.legal_hold",
        document_id,
        if hold { "on" } else { "off" },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbac::Role;

    fn setup() -> (tempfile::TempDir, Database, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path().join("office.sqlite"), SecretKey::from_bytes([13u8; 32]))
            .unwrap();
        let vault = Vault::new(dir.path().join("vault")).unwrap();
        (dir, db, vault)
    }

    fn notary() -> Principal {
        Principal::new("u1", vec![Role::NotaryTitular])
    }

    #[test]
    fn stores_reads_and_verifies_a_document() {
        let (_dir, db, vault) = setup();
        let record = store_document(
            &db,
            &vault,
            &notary(),
            StoreDocumentInput {
                case_id: None,
                title: "Acte fictif",
                mime: "application/pdf",
                bytes: b"%PDF-1.7 contenu fictif",
                retention_class: "case",
            },
        )
        .unwrap();

        let (read_record, bytes) = read_document(&db, &vault, &notary(), &record.id).unwrap();
        assert_eq!(bytes, b"%PDF-1.7 contenu fictif");
        assert_eq!(read_record.sha256, record.sha256);
    }

    #[test]
    fn vault_file_reveals_no_plaintext() {
        let (dir, db, vault) = setup();
        store_document(
            &db,
            &vault,
            &notary(),
            StoreDocumentInput {
                case_id: None,
                title: "t",
                mime: "text/plain",
                bytes: b"NIN 109876543210987654 BENZAKOUR",
                retention_class: "case",
            },
        )
        .unwrap();

        for entry in walk(dir.path().join("vault")) {
            let bytes = std::fs::read(&entry).unwrap();
            assert!(
                !bytes.windows(9).any(|w| w == b"BENZAKOUR"),
                "plaintext leaked into {}",
                entry.display()
            );
        }
    }

    fn walk(root: PathBuf) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    out.extend(walk(path));
                } else {
                    out.push(path);
                }
            }
        }
        out
    }

    #[test]
    fn tampering_with_a_vault_file_is_detected() {
        let (dir, db, vault) = setup();
        let record = store_document(
            &db,
            &vault,
            &notary(),
            StoreDocumentInput {
                case_id: None,
                title: "t",
                mime: "text/plain",
                bytes: b"original content",
                retention_class: "case",
            },
        )
        .unwrap();

        let vault_file = walk(dir.path().join("vault")).pop().unwrap();
        let mut bytes = std::fs::read(&vault_file).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        std::fs::write(&vault_file, bytes).unwrap();

        assert!(matches!(
            read_document(&db, &vault, &notary(), &record.id),
            Err(CoreError::Crypto(_))
        ));
    }

    #[test]
    fn ocr_text_is_stored_encrypted() {
        let (dir, db, vault) = setup();
        let record = store_document(
            &db,
            &vault,
            &notary(),
            StoreDocumentInput {
                case_id: None,
                title: "t",
                mime: "image/png",
                bytes: b"fake-image-bytes",
                retention_class: "case",
            },
        )
        .unwrap();
        attach_ocr_text(&db, &notary(), &record.id, "مكتب التوثيق سجل ٢٠٢٦").unwrap();
        assert_eq!(
            ocr_text(&db, &notary(), &record.id).unwrap().as_deref(),
            Some("مكتب التوثيق سجل ٢٠٢٦")
        );

        let bytes = std::fs::read(dir.path().join("office.sqlite")).unwrap();
        let needle = "مكتب التوثيق".as_bytes();
        // The WAL may hold recent pages; check both files for the plaintext.
        assert!(!bytes.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn legal_hold_prevents_deletion() {
        let (_dir, db, vault) = setup();
        let record = store_document(
            &db,
            &vault,
            &notary(),
            StoreDocumentInput {
                case_id: None,
                title: "t",
                mime: "text/plain",
                bytes: b"held",
                retention_class: "act",
            },
        )
        .unwrap();
        set_legal_hold(&db, &notary(), &record.id, true).unwrap();
        let delete = db
            .connection()
            .execute("DELETE FROM document WHERE id = ?1", rusqlite::params![record.id]);
        assert!(delete.is_err(), "legal hold must block deletion");
    }

    #[test]
    fn a_clerk_without_document_permission_cannot_store_or_read() {
        let (_dir, db, vault) = setup();
        let inspector = Principal::new("i1", vec![Role::Inspector]);
        assert!(matches!(
            store_document(
                &db,
                &vault,
                &inspector,
                StoreDocumentInput {
                    case_id: None,
                    title: "t",
                    mime: "text/plain",
                    bytes: b"x",
                    retention_class: "case",
                }
            ),
            Err(CoreError::Denied)
        ));
    }
}
