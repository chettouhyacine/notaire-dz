//! Registers, minutes and copy issuance.
//!
//! A number is allocated in the same transaction that writes the register entry, and the entry
//! carries a hash chained to the previous one in that register. The database refuses to update or
//! delete an entry, so a register cannot be quietly renumbered: corrections are new, linked entries.

use serde::{Deserialize, Serialize};

use crate::casefile::Stage;
use crate::error::{CoreError, Result};
use crate::party::new_id;
use crate::rbac::{Permission, Principal};
use crate::storage::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterInfo {
    pub id: String,
    pub code: String,
    pub series_prefix: String,
    pub year: i32,
    pub next_seq: i64,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterEntry {
    pub id: String,
    pub register_id: String,
    pub seq: i64,
    pub act_reference: String,
    pub case_id: Option<String>,
    pub finalised_at_ms: i64,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Minute {
    pub id: String,
    pub act_reference: String,
    pub room: String,
    pub shelf: String,
    pub box_name: String,
    pub on_loan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyIssued {
    pub id: String,
    pub act_reference: String,
    pub copy_kind: String,
    pub copy_number: i64,
    pub watermark: String,
}

pub fn ensure_register(
    db: &Database,
    office_id: &str,
    code: &str,
    series_prefix: &str,
    year: i32,
) -> Result<String> {
    if code.trim().is_empty() {
        return Err(CoreError::Validation("register code is required".into()));
    }
    let existing: Option<String> = db
        .connection()
        .query_row(
            "SELECT id FROM register WHERE office_id = ?1 AND code = ?2 AND year = ?3",
            rusqlite::params![office_id, code, year],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = new_id("reg")?;
    db.connection().execute(
        "INSERT INTO register (id, code, series_prefix, year, next_seq, office_id, closed_at_ms)
         VALUES (?1, ?2, ?3, ?4, 1, ?5, NULL)",
        rusqlite::params![id, code.trim(), series_prefix, year, office_id],
    )?;
    Ok(id)
}

pub fn register_info(db: &Database, register_id: &str) -> Result<RegisterInfo> {
    db.connection()
        .query_row(
            "SELECT id, code, series_prefix, year, next_seq, closed_at_ms FROM register WHERE id = ?1",
            rusqlite::params![register_id],
            |r| {
                Ok(RegisterInfo {
                    id: r.get(0)?,
                    code: r.get(1)?,
                    series_prefix: r.get(2)?,
                    year: r.get(3)?,
                    next_seq: r.get(4)?,
                    closed: r.get::<_, Option<i64>>(5)?.is_some(),
                })
            },
        )
        .map_err(|_| CoreError::Validation("register not found".into()))
}

fn last_entry_hash(db: &Database, register_id: &str) -> Result<String> {
    let last: Option<String> = db
        .connection()
        .query_row(
            "SELECT entry_hash FROM register_entry WHERE register_id = ?1 ORDER BY seq DESC LIMIT 1",
            rusqlite::params![register_id],
            |r| r.get(0),
        )
        .ok();
    Ok(last.unwrap_or_else(|| "genesis".to_string()))
}

/// Allocate the next number for a finalised act. Notary-only, and only after signature.
pub fn allocate_number(
    db: &Database,
    principal: &Principal,
    register_id: &str,
    case_id: Option<&str>,
    act_reference: &str,
) -> Result<RegisterEntry> {
    principal.require_notary(Permission::ActFinalise)?;
    if act_reference.trim().is_empty() {
        return Err(CoreError::Validation("an act reference is required".into()));
    }

    if let Some(case_id) = case_id {
        let acl = crate::casefile::acl_for(db, case_id)?;
        crate::rbac::require_case(principal, Permission::ActFinalise, &acl)?;
        let stage: String = db
            .connection()
            .query_row(
                "SELECT stage FROM case_file WHERE id = ?1",
                rusqlite::params![case_id],
                |r| r.get(0),
            )
            .map_err(|_| CoreError::Validation("case not found".into()))?;
        let stage = Stage::from_code(&stage)?;
        if stage < Stage::Signature {
            return Err(CoreError::Validation(
                "an act cannot be numbered before it is signed".into(),
            ));
        }
    }

    let tx = db.connection().unchecked_transaction()?;
    let (next_seq, closed): (i64, Option<i64>) = tx
        .query_row(
            "SELECT next_seq, closed_at_ms FROM register WHERE id = ?1",
            rusqlite::params![register_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| CoreError::Validation("register not found".into()))?;
    if closed.is_some() {
        return Err(CoreError::Validation("register is closed".into()));
    }

    let previous = last_entry_hash(db, register_id)?;
    let now = crate::now_ms();
    let id = new_id("rent")?;
    let entry_hash = crate::crypto::sha256_hex(
        format!("{previous}|{next_seq}|{act_reference}|{now}").as_bytes(),
    );

    tx.execute(
        "INSERT INTO register_entry (id, register_id, seq, act_reference, case_id,
                                     finalised_at_ms, actor_id, entry_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            register_id,
            next_seq,
            act_reference.trim(),
            case_id,
            now,
            principal.user_id,
            entry_hash
        ],
    )?;
    tx.execute(
        "UPDATE register SET next_seq = ?2 WHERE id = ?1",
        rusqlite::params![register_id, next_seq + 1],
    )?;
    tx.commit()?;

    db.append_audit(
        &principal.user_id,
        "register.allocate",
        register_id,
        &format!("seq={next_seq}"),
    )?;

    Ok(RegisterEntry {
        id,
        register_id: register_id.to_string(),
        seq: next_seq,
        act_reference: act_reference.trim().to_string(),
        case_id: case_id.map(str::to_string),
        finalised_at_ms: now,
        entry_hash,
    })
}

pub fn entries(db: &Database, register_id: &str) -> Result<Vec<RegisterEntry>> {
    let mut stmt = db.connection().prepare(
        "SELECT id, register_id, seq, act_reference, case_id, finalised_at_ms, entry_hash
         FROM register_entry WHERE register_id = ?1 ORDER BY seq",
    )?;
    let rows = stmt.query_map(rusqlite::params![register_id], |r| {
        Ok(RegisterEntry {
            id: r.get(0)?,
            register_id: r.get(1)?,
            seq: r.get(2)?,
            act_reference: r.get(3)?,
            case_id: r.get(4)?,
            finalised_at_ms: r.get(5)?,
            entry_hash: r.get(6)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

/// Verify the per-register hash chain and the gap-free sequence.
pub fn verify_register(db: &Database, register_id: &str) -> Result<()> {
    let entries = entries(db, register_id)?;
    let mut expected_seq = 1i64;
    let mut previous = "genesis".to_string();
    for entry in entries {
        if entry.seq != expected_seq {
            return Err(CoreError::Validation(format!(
                "register gap at sequence {expected_seq}"
            )));
        }
        let recomputed = crate::crypto::sha256_hex(
            format!(
                "{previous}|{}|{}|{}",
                entry.seq, entry.act_reference, entry.finalised_at_ms
            )
            .as_bytes(),
        );
        if recomputed != entry.entry_hash {
            return Err(CoreError::Validation(format!(
                "register chain broken at sequence {}",
                entry.seq
            )));
        }
        previous = entry.entry_hash;
        expected_seq += 1;
    }
    Ok(())
}

pub fn close_register(db: &Database, principal: &Principal, register_id: &str) -> Result<()> {
    principal.require(RegisterClosePermission)?;
    verify_register(db, register_id)?;
    db.connection().execute(
        "UPDATE register SET closed_at_ms = ?2 WHERE id = ?1",
        rusqlite::params![register_id, crate::now_ms()],
    )?;
    db.append_audit(&principal.user_id, "register.close", register_id, "")?;
    Ok(())
}

/// Alias so the permission reads well at the call site.
#[allow(non_upper_case_globals)]
const RegisterClosePermission: Permission = Permission::RegisterClose;

// ---------- minutes ----------

pub fn store_minute(
    db: &Database,
    principal: &Principal,
    act_reference: &str,
    case_id: Option<&str>,
    room: &str,
    shelf: &str,
    box_name: &str,
) -> Result<Minute> {
    principal.require(Permission::DocumentWrite)?;
    if act_reference.trim().is_empty() {
        return Err(CoreError::Validation("an act reference is required".into()));
    }
    let id = new_id("min")?;
    db.connection().execute(
        "INSERT INTO minute (id, act_reference, case_id, room_enc, shelf_enc, box_enc, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            act_reference.trim(),
            case_id,
            db.encrypt_field(room, &format!("minute:{id}:room"))?,
            db.encrypt_field(shelf, &format!("minute:{id}:shelf"))?,
            db.encrypt_field(box_name, &format!("minute:{id}:box"))?,
            crate::now_ms()
        ],
    )?;
    db.append_audit(&principal.user_id, "minute.store", &id, act_reference)?;
    Ok(Minute {
        id,
        act_reference: act_reference.trim().to_string(),
        room: room.to_string(),
        shelf: shelf.to_string(),
        box_name: box_name.to_string(),
        on_loan: false,
    })
}

pub fn loans_minute(
    db: &Database,
    principal: &Principal,
    minute_id: &str,
    borrower: &str,
    reason: &str,
) -> Result<()> {
    principal.require(Permission::RegisterRead)?;
    let open: Option<String> = db
        .connection()
        .query_row(
            "SELECT id FROM minute_loan WHERE minute_id = ?1 AND returned_at_ms IS NULL",
            rusqlite::params![minute_id],
            |r| r.get(0),
        )
        .ok();
    if open.is_some() {
        return Err(CoreError::Validation("the minute is already out on loan".into()));
    }
    db.connection().execute(
        "INSERT INTO minute_loan (id, minute_id, borrower_enc, reason_enc, borrowed_at_ms,
                                  returned_at_ms, actor_id)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
        rusqlite::params![
            new_id("loan")?,
            minute_id,
            db.encrypt_field(borrower, &format!("minute:{minute_id}:borrower"))?,
            db.encrypt_field(reason, &format!("minute:{minute_id}:reason"))?,
            crate::now_ms(),
            principal.user_id
        ],
    )?;
    db.append_audit(&principal.user_id, "minute.loan", minute_id, "")?;
    Ok(())
}

pub fn return_minute(db: &Database, principal: &Principal, minute_id: &str) -> Result<()> {
    principal.require(Permission::RegisterRead)?;
    let updated = db.connection().execute(
        "UPDATE minute_loan SET returned_at_ms = ?2
         WHERE minute_id = ?1 AND returned_at_ms IS NULL",
        rusqlite::params![minute_id, crate::now_ms()],
    )?;
    if updated == 0 {
        return Err(CoreError::Validation("this minute is not on loan".into()));
    }
    db.append_audit(&principal.user_id, "minute.return", minute_id, "")?;
    Ok(())
}

pub fn minute_is_on_loan(db: &Database, minute_id: &str) -> Result<bool> {
    let open: Option<String> = db
        .connection()
        .query_row(
            "SELECT id FROM minute_loan WHERE minute_id = ?1 AND returned_at_ms IS NULL",
            rusqlite::params![minute_id],
            |r| r.get(0),
        )
        .ok();
    Ok(open.is_some())
}

// ---------- copies ----------

/// Issue a copy. Each copy gets its own sequence per act and per kind, and a watermark string that
/// carries the act reference, kind and number so a leaked copy can be traced back.
pub fn issue_copy(
    db: &Database,
    principal: &Principal,
    act_reference: &str,
    case_id: Option<&str>,
    kind: &str,
    recipient: &str,
) -> Result<CopyIssued> {
    principal.require(Permission::ActIssueCopy)?;
    if !matches!(kind, "simple" | "authentic" | "executory") {
        return Err(CoreError::Validation("unknown copy kind".into()));
    }
    if recipient.trim().is_empty() {
        return Err(CoreError::Validation("the recipient must be recorded".into()));
    }
    if act_reference.trim().is_empty() {
        return Err(CoreError::Validation("an act reference is required".into()));
    }

    let next: i64 = db
        .connection()
        .query_row(
            "SELECT COALESCE(MAX(copy_number), 0) + 1 FROM copy_issued
             WHERE act_reference = ?1 AND copy_kind = ?2",
            rusqlite::params![act_reference.trim(), kind],
            |r| r.get(0),
        )?;

    let id = new_id("copy")?;
    let watermark = format!("{}·{}·{}", act_reference.trim(), kind, next);
    db.connection().execute(
        "INSERT INTO copy_issued (id, case_id, act_reference, copy_kind, copy_number,
                                  recipient_enc, issued_at_ms, issued_by, watermark)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            id,
            case_id,
            act_reference.trim(),
            kind,
            next,
            db.encrypt_field(recipient, &format!("copy:{id}:recipient"))?,
            crate::now_ms(),
            principal.user_id,
            watermark
        ],
    )?;
    db.append_audit(&principal.user_id, "copy.issue", &id, kind)?;

    Ok(CopyIssued {
        id,
        act_reference: act_reference.trim().to_string(),
        copy_kind: kind.to_string(),
        copy_number: next,
        watermark,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretKey;
    use crate::rbac::Role;

    fn setup() -> (tempfile::TempDir, Database, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path().join("office.sqlite"), SecretKey::from_bytes([14u8; 32]))
            .unwrap();
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        // A case that has reached signature, so numbering is legal.
        let case_id = {
            let c = crate::casefile::create_case(&db, &notary, "sale", "u1", false, None, None).unwrap();
            for stage in [
                Stage::Documents,
                Stage::Verification,
                Stage::Drafting,
                Stage::Review,
                Stage::Reading,
                Stage::Signature,
            ] {
                crate::casefile::advance_stage(&db, &notary, &c, stage).unwrap();
            }
            c
        };
        let register = ensure_register(&db, "office-1", "repertoire", "R", 2026).unwrap();
        (dir, db, case_id, register)
    }

    #[test]
    fn numbering_is_sequential_gap_free_and_chained() {
        let (_dir, db, case_id, register) = setup();
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);

        let first = allocate_number(&db, &notary, &register, Some(&case_id), "2026/0001").unwrap();
        let second = allocate_number(&db, &notary, &register, Some(&case_id), "2026/0002").unwrap();
        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        assert_ne!(first.entry_hash, second.entry_hash);

        verify_register(&db, &register).unwrap();
        assert_eq!(register_info(&db, &register).unwrap().next_seq, 3);
    }

    #[test]
    fn a_clerk_cannot_allocate_a_number() {
        let (_dir, db, case_id, register) = setup();
        let clerk = Principal::new("u2", vec![Role::Clerk]);
        assert!(matches!(
            allocate_number(&db, &clerk, &register, Some(&case_id), "2026/0001"),
            Err(CoreError::Denied)
        ));
    }

    #[test]
    fn an_unsigned_act_cannot_be_numbered() {
        let (_dir, db, _case_id, register) = setup();
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        let fresh = crate::casefile::create_case(&db, &notary, "sale", "u1", false, None, None).unwrap();
        let error = allocate_number(&db, &notary, &register, Some(&fresh), "2026/9999").unwrap_err();
        assert!(matches!(error, CoreError::Validation(_)));
    }

    #[test]
    fn register_entries_cannot_be_edited_or_removed() {
        let (_dir, db, case_id, register) = setup();
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        allocate_number(&db, &notary, &register, Some(&case_id), "2026/0001").unwrap();

        assert!(db
            .connection()
            .execute("UPDATE register_entry SET act_reference = 'x'", [])
            .is_err());
        assert!(db.connection().execute("DELETE FROM register_entry", []).is_err());
    }

    #[test]
    fn tampering_with_an_entry_breaks_verification() {
        let (_dir, db, case_id, register) = setup();
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        allocate_number(&db, &notary, &register, Some(&case_id), "2026/0001").unwrap();

        // Simulate an attacker with database access who disables the trigger, then edits a row.
        db.connection()
            .execute_batch(
                "DROP TRIGGER register_entry_no_update;
                 UPDATE register_entry SET act_reference = 'FORGED';",
            )
            .unwrap();
        assert!(verify_register(&db, &register).is_err());
    }

    #[test]
    fn closed_registers_refuse_new_numbers() {
        let (_dir, db, case_id, register) = setup();
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        allocate_number(&db, &notary, &register, Some(&case_id), "2026/0001").unwrap();
        close_register(&db, &notary, &register).unwrap();
        assert!(allocate_number(&db, &notary, &register, Some(&case_id), "2026/0002").is_err());
    }

    #[test]
    fn minute_loan_lifecycle_is_tracked() {
        let (_dir, db, case_id, _register) = setup();
        let clerk = Principal::new("u2", vec![Role::Clerk]);
        let minute = store_minute(&db, &clerk, "2026/0001", Some(&case_id), "Salle A", "Étagère 3", "Boîte 12")
            .unwrap();
        assert!(!minute_is_on_loan(&db, &minute.id).unwrap());

        loans_minute(&db, &clerk, &minute.id, "Maître FICTIF", "Consultation").unwrap();
        assert!(minute_is_on_loan(&db, &minute.id).unwrap());
        // A second loan for the same minute is refused until it comes back.
        assert!(loans_minute(&db, &clerk, &minute.id, "Autre", "x").is_err());

        return_minute(&db, &clerk, &minute.id).unwrap();
        assert!(!minute_is_on_loan(&db, &minute.id).unwrap());

        // Minutes are never destroyed.
        assert!(db.connection().execute("DELETE FROM minute", []).is_err());
    }

    #[test]
    fn copies_are_numbered_per_act_and_kind_with_a_watermark() {
        let (_dir, db, case_id, _register) = setup();
        let clerk = Principal::new("u2", vec![Role::Clerk]);

        let a = issue_copy(&db, &clerk, "2026/0001", Some(&case_id), "simple", "M. FICTIF").unwrap();
        let b = issue_copy(&db, &clerk, "2026/0001", Some(&case_id), "simple", "Mme FICTIVE").unwrap();
        let c = issue_copy(&db, &clerk, "2026/0001", Some(&case_id), "executory", "Banque FICTIVE").unwrap();

        assert_eq!(a.copy_number, 1);
        assert_eq!(b.copy_number, 2);
        assert_eq!(c.copy_number, 1, "executory copies have their own sequence");
        assert!(a.watermark.contains("2026/0001"));
        assert!(a.watermark.contains("simple"));

        // An issued copy is a legal record.
        assert!(db
            .connection()
            .execute("UPDATE copy_issued SET recipient_enc = X'00'", [])
            .is_err());
        assert!(issue_copy(&db, &clerk, "2026/0001", None, "simple", "").is_err());
    }
}
