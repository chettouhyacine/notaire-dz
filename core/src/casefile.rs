//! Case files: lifecycle stages, checklists and per-case access control.
//!
//! A case moves forward through a fixed stage order; each move is recorded as an event (who, when)
//! rather than overwriting a status field, so the file keeps its own history. Checklist items are
//! instantiated from a versioned template and carry their `[VERIFY]` review status with them.

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::party::new_id;
use crate::rbac::{require_case, CaseAcl, Permission, Principal};
use crate::storage::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Stage {
    Intake,
    Documents,
    Verification,
    Drafting,
    Review,
    Reading,
    Signature,
    Registration,
    Publication,
    Delivery,
    Archived,
}

impl Stage {
    pub fn code(self) -> &'static str {
        match self {
            Stage::Intake => "intake",
            Stage::Documents => "documents",
            Stage::Verification => "verification",
            Stage::Drafting => "drafting",
            Stage::Review => "review",
            Stage::Reading => "reading",
            Stage::Signature => "signature",
            Stage::Registration => "registration",
            Stage::Publication => "publication",
            Stage::Delivery => "delivery",
            Stage::Archived => "archived",
        }
    }

    pub fn from_code(code: &str) -> Result<Self> {
        Ok(match code {
            "intake" => Stage::Intake,
            "documents" => Stage::Documents,
            "verification" => Stage::Verification,
            "drafting" => Stage::Drafting,
            "review" => Stage::Review,
            "reading" => Stage::Reading,
            "signature" => Stage::Signature,
            "registration" => Stage::Registration,
            "publication" => Stage::Publication,
            "delivery" => Stage::Delivery,
            "archived" => Stage::Archived,
            other => return Err(CoreError::Validation(format!("unknown stage {other}"))),
        })
    }

    pub fn all() -> &'static [Stage] {
        &[
            Stage::Intake,
            Stage::Documents,
            Stage::Verification,
            Stage::Drafting,
            Stage::Review,
            Stage::Reading,
            Stage::Signature,
            Stage::Registration,
            Stage::Publication,
            Stage::Delivery,
            Stage::Archived,
        ]
    }

    /// Forward by one, or straight back by one. No jumping over a step: an act cannot reach
    /// registration before signature.
    pub fn can_move_to(self, next: Stage) -> bool {
        let order = Stage::all();
        let from = order.iter().position(|s| *s == self);
        let to = order.iter().position(|s| *s == next);
        match (from, to) {
            (Some(f), Some(t)) => t == f + 1 || (t + 1 == f),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistSeedItem {
    pub code: String,
    pub label_key: String,
    pub required: bool,
    pub ordinal: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistTemplateSpec {
    pub act_type: String,
    pub version_no: u32,
    pub source: String,
    pub status: String,
    pub items: Vec<ChecklistSeedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItemView {
    pub id: String,
    pub code: String,
    pub label_key: String,
    pub required: bool,
    pub status: String,
    pub document_id: Option<String>,
    pub expires_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseSummary {
    pub id: String,
    pub reference: String,
    pub act_type: String,
    pub stage: String,
    pub owner_user_id: String,
    pub opened_at_ms: i64,
    pub due_at_ms: Option<i64>,
    pub confidential: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseDetail {
    pub summary: CaseSummary,
    pub parties: Vec<String>,
    pub checklist: Vec<ChecklistItemView>,
    pub missing_documents: Vec<String>,
    pub stages: Vec<(String, i64)>,
}

fn year_of(now_ms: i64) -> i32 {
    // Reference years follow UTC so a reference never changes with the local clock.
    let days = (now_ms / 86_400_000) as i32;
    crate::date::Date::from_days(days).to_ymd().0
}

fn next_case_reference(db: &Database, year: i32) -> Result<String> {
    let tx = db.connection().unchecked_transaction()?;
    let current: Option<i64> = tx
        .query_row(
            "SELECT next_seq FROM case_counter WHERE year = ?1",
            rusqlite::params![year],
            |r| r.get(0),
        )
        .ok();
    let seq = current.unwrap_or(1);
    tx.execute(
        "INSERT INTO case_counter (year, next_seq) VALUES (?1, ?2)
         ON CONFLICT(year) DO UPDATE SET next_seq = ?2",
        rusqlite::params![year, seq + 1],
    )?;
    tx.commit()?;
    Ok(format!("{year}/{seq:04}"))
}

pub fn create_case(
    db: &Database,
    principal: &Principal,
    act_type: &str,
    owner_user_id: &str,
    confidential: bool,
    due_at_ms: Option<i64>,
    template: Option<&ChecklistTemplateSpec>,
) -> Result<String> {
    principal.require(Permission::CaseCreate)?;
    if act_type.trim().is_empty() {
        return Err(CoreError::Validation("act type is required".into()));
    }
    let id = new_id("case")?;
    let now = crate::now_ms();
    let year = year_of(now);
    let reference = next_case_reference(db, year)?;

    let tx = db.connection().unchecked_transaction()?;
    tx.execute(
        "INSERT INTO case_file (id, reference, act_type, stage, owner_user_id, opened_at_ms,
                                due_at_ms, confidential, notes_enc, created_by)
         VALUES (?1, ?2, ?3, 'intake', ?4, ?5, ?6, ?7, NULL, ?8)",
        rusqlite::params![
            id,
            reference,
            act_type.trim(),
            owner_user_id,
            now,
            due_at_ms,
            confidential as i64,
            principal.user_id
        ],
    )?;
    tx.execute(
        "INSERT INTO case_stage_event (id, case_id, stage, entered_at_ms, actor_id, note_enc)
         VALUES (?1, ?2, 'intake', ?3, ?4, NULL)",
        rusqlite::params![new_id("stage")?, id, now, principal.user_id],
    )?;
    tx.commit()?;

    if confidential {
        db.connection().execute(
            "INSERT OR IGNORE INTO case_acl (case_id, user_id, granted_at_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, principal.user_id, now],
        )?;
    }

    if let Some(spec) = template {
        attach_checklist(db, principal, &id, spec)?;
    }

    db.append_audit(&principal.user_id, "case.create", &id, act_type)?;
    Ok(id)
}

pub fn attach_checklist(
    db: &Database,
    principal: &Principal,
    case_id: &str,
    spec: &ChecklistTemplateSpec,
) -> Result<()> {
    principal.require(Permission::CaseWrite)?;
    if spec.items.iter().all(|i| i.code.trim().is_empty()) {
        return Err(CoreError::Validation("checklist template has no items".into()));
    }
    let template_id = new_id("tpl")?;
    let instance_id = new_id("chk")?;
    let tx = db.connection().unchecked_transaction()?;
    tx.execute(
        "INSERT INTO checklist_template (id, act_type, version_no, source, status)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            template_id,
            spec.act_type,
            spec.version_no as i64,
            spec.source,
            spec.status
        ],
    )?;
    for item in &spec.items {
        tx.execute(
            "INSERT INTO checklist_template_item (id, template_id, code, label_key, required, ordinal)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                new_id("tpli")?,
                template_id,
                item.code,
                item.label_key,
                item.required as i64,
                item.ordinal as i64
            ],
        )?;
    }
    tx.execute(
        "INSERT INTO checklist_instance (id, case_id, template_id, created_at_ms)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![instance_id, case_id, template_id, crate::now_ms()],
    )?;
    for item in &spec.items {
        tx.execute(
            "INSERT INTO checklist_item (id, instance_id, code, label_key, required, status,
                                         document_id, expires_on, note_enc)
             VALUES (?1, ?2, ?3, ?4, ?5, 'missing', NULL, NULL, NULL)",
            rusqlite::params![
                new_id("chki")?,
                instance_id,
                item.code,
                item.label_key,
                item.required as i64
            ],
        )?;
    }
    tx.commit()?;
    db.append_audit(&principal.user_id, "case.checklist_attach", case_id, &spec.act_type)?;
    Ok(())
}

pub fn advance_stage(
    db: &Database,
    principal: &Principal,
    case_id: &str,
    to: Stage,
) -> Result<()> {
    let acl = acl_for(db, case_id)?;
    require_case(principal, Permission::CaseWrite, &acl)?;

    let current: String = db
        .connection()
        .query_row(
            "SELECT stage FROM case_file WHERE id = ?1",
            rusqlite::params![case_id],
            |r| r.get(0),
        )
        .map_err(|_| CoreError::Validation("case not found".into()))?;
    let from = Stage::from_code(&current)?;
    if !from.can_move_to(to) {
        return Err(CoreError::Validation(format!(
            "illegal stage transition {} -> {}",
            from.code(),
            to.code()
        )));
    }
    if to == Stage::Signature {
        principal.require_notary(Permission::ActFinalise)?;
    }

    let tx = db.connection().unchecked_transaction()?;
    tx.execute(
        "UPDATE case_file SET stage = ?2 WHERE id = ?1",
        rusqlite::params![case_id, to.code()],
    )?;
    tx.execute(
        "INSERT INTO case_stage_event (id, case_id, stage, entered_at_ms, actor_id, note_enc)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        rusqlite::params![new_id("stage")?, case_id, to.code(), crate::now_ms(), principal.user_id],
    )?;
    tx.commit()?;
    db.append_audit(
        &principal.user_id,
        "case.stage",
        case_id,
        &format!("{}->{}", from.code(), to.code()),
    )?;
    Ok(())
}

pub fn acl_for(db: &Database, case_id: &str) -> Result<CaseAcl> {
    let confidential: bool = db
        .connection()
        .query_row(
            "SELECT confidential FROM case_file WHERE id = ?1",
            rusqlite::params![case_id],
            |r| r.get::<_, i64>(0).map(|v| v != 0),
        )
        .map_err(|_| CoreError::Validation("case not found".into()))?;
    let mut stmt =
        db.connection()
            .prepare("SELECT user_id FROM case_acl WHERE case_id = ?1")?;
    let users = stmt
        .query_map(rusqlite::params![case_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(CaseAcl { case_id: case_id.to_string(), confidential, allowed_user_ids: users })
}

pub fn get_case(db: &Database, principal: &Principal, case_id: &str) -> Result<CaseDetail> {
    let acl = acl_for(db, case_id)?;
    require_case(principal, Permission::CaseRead, &acl)?;

    let summary = db.connection().query_row(
        "SELECT id, reference, act_type, stage, owner_user_id, opened_at_ms, due_at_ms, confidential
         FROM case_file WHERE id = ?1",
        rusqlite::params![case_id],
        |r| {
            Ok(CaseSummary {
                id: r.get(0)?,
                reference: r.get(1)?,
                act_type: r.get(2)?,
                stage: r.get(3)?,
                owner_user_id: r.get(4)?,
                opened_at_ms: r.get(5)?,
                due_at_ms: r.get(6)?,
                confidential: r.get::<_, i64>(7)? != 0,
            })
        },
    )?;

    let mut stmt =
        db.connection()
            .prepare("SELECT party_id FROM case_party WHERE case_id = ?1 ORDER BY role")?;
    let parties = stmt
        .query_map(rusqlite::params![case_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut stmt = db.connection().prepare(
        "SELECT i.id, i.code, i.label_key, i.required, i.status, i.document_id, i.expires_on
         FROM checklist_item i
         JOIN checklist_instance c ON c.id = i.instance_id
         WHERE c.case_id = ?1
         ORDER BY i.code",
    )?;
    let checklist = stmt
        .query_map(rusqlite::params![case_id], |r| {
            Ok(ChecklistItemView {
                id: r.get(0)?,
                code: r.get(1)?,
                label_key: r.get(2)?,
                required: r.get::<_, i64>(3)? != 0,
                status: r.get(4)?,
                document_id: r.get(5)?,
                expires_on: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut stmt = db
        .connection()
        .prepare("SELECT stage, entered_at_ms FROM case_stage_event WHERE case_id = ?1 ORDER BY entered_at_ms")?;
    let stages = stmt
        .query_map(rusqlite::params![case_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let missing_documents = checklist
        .iter()
        .filter(|i| i.required && i.status != "received")
        .map(|i| i.code.clone())
        .collect();

    Ok(CaseDetail { summary, parties, checklist, missing_documents, stages })
}

/// Cases the principal is allowed to see, newest first. Confidential cases are filtered by ACL
/// here, not in the UI.
pub fn list_cases(db: &Database, principal: &Principal, limit: usize) -> Result<Vec<CaseSummary>> {
    principal.require(Permission::CaseRead)?;
    let mut stmt = db.connection().prepare(
        "SELECT id, reference, act_type, stage, owner_user_id, opened_at_ms, due_at_ms, confidential
         FROM case_file
         ORDER BY opened_at_ms DESC, reference DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![limit as i64], |r| {
        Ok(CaseSummary {
            id: r.get(0)?,
            reference: r.get(1)?,
            act_type: r.get(2)?,
            stage: r.get(3)?,
            owner_user_id: r.get(4)?,
            opened_at_ms: r.get(5)?,
            due_at_ms: r.get(6)?,
            confidential: r.get::<_, i64>(7)? != 0,
        })
    })?;

    let mut visible = Vec::new();
    for row in rows {
        let summary = row?;
        if !summary.confidential {
            visible.push(summary);
            continue;
        }
        let acl = acl_for(db, &summary.id)?;
        if acl.allows(&principal.user_id) {
            visible.push(summary);
        }
    }
    Ok(visible)
}

/// Cases with a due date inside the next `days` and not yet archived: the Home card.
pub fn approaching_deadlines(db: &Database, now_ms: i64, days: i64) -> Result<Vec<CaseSummary>> {
    let horizon = now_ms + days * 86_400_000;
    let mut stmt = db.connection().prepare(
        "SELECT id, reference, act_type, stage, owner_user_id, opened_at_ms, due_at_ms, confidential
         FROM case_file
         WHERE due_at_ms IS NOT NULL AND due_at_ms <= ?1 AND stage != 'archived'
         ORDER BY due_at_ms",
    )?;
    let rows = stmt.query_map(rusqlite::params![horizon], |r| {
        Ok(CaseSummary {
            id: r.get(0)?,
            reference: r.get(1)?,
            act_type: r.get(2)?,
            stage: r.get(3)?,
            owner_user_id: r.get(4)?,
            opened_at_ms: r.get(5)?,
            due_at_ms: r.get(6)?,
            confidential: r.get::<_, i64>(7)? != 0,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretKey;
    use crate::rbac::Role;

    fn db(dir: &std::path::Path) -> Database {
        Database::open(dir.join("office.sqlite"), SecretKey::from_bytes([12u8; 32])).unwrap()
    }

    fn template() -> ChecklistTemplateSpec {
        ChecklistTemplateSpec {
            act_type: "sale".into(),
            version_no: 1,
            source: "DEMO ONLY".into(),
            status: "verify".into(),
            items: vec![
                ChecklistSeedItem {
                    code: "title".into(),
                    label_key: "checklist.sale.title".into(),
                    required: true,
                    ordinal: 1,
                },
                ChecklistSeedItem {
                    code: "mortgage_cert".into(),
                    label_key: "checklist.sale.mortgage_cert".into(),
                    required: true,
                    ordinal: 2,
                },
            ],
        }
    }

    #[test]
    fn creates_cases_with_gap_free_references() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(dir.path());
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        let a = create_case(&db, &notary, "sale", "u1", false, None, None).unwrap();
        let b = create_case(&db, &notary, "sale", "u1", false, None, None).unwrap();
        let ra = get_case(&db, &notary, &a).unwrap().summary.reference;
        let rb = get_case(&db, &notary, &b).unwrap().summary.reference;
        assert!(ra.ends_with("/0001"));
        assert!(rb.ends_with("/0002"));
    }

    #[test]
    fn checklist_is_instantiated_and_missing_documents_are_listed() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(dir.path());
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        let case_id = create_case(&db, &notary, "sale", "u1", false, None, Some(&template())).unwrap();

        let detail = get_case(&db, &notary, &case_id).unwrap();
        assert_eq!(detail.checklist.len(), 2);
        assert!(detail.checklist.iter().all(|i| i.status == "missing"));
        assert_eq!(detail.missing_documents.len(), 2);

        // Mark one item received: the missing list shrinks, the item keeps its identity.
        db.connection()
            .execute(
                "UPDATE checklist_item SET status = 'received' WHERE code = 'title'",
                [],
            )
            .unwrap();
        let after = get_case(&db, &notary, &case_id).unwrap();
        assert_eq!(after.missing_documents, vec!["mortgage_cert".to_string()]);
    }

    #[test]
    fn stages_move_one_step_at_a_time_and_signature_needs_a_notary() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(dir.path());
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        let clerk = Principal::new("u2", vec![Role::Clerk]);
        let case_id = create_case(&db, &notary, "sale", "u1", false, None, None).unwrap();

        // Jumping over stages is refused.
        assert!(advance_stage(&db, &notary, &case_id, Stage::Signature).is_err());

        for stage in [Stage::Documents, Stage::Verification, Stage::Drafting, Stage::Review, Stage::Reading] {
            advance_stage(&db, &notary, &case_id, stage).unwrap();
        }
        // A clerk cannot move a case to signature.
        assert!(matches!(
            advance_stage(&db, &clerk, &case_id, Stage::Signature),
            Err(CoreError::Denied)
        ));
        advance_stage(&db, &notary, &case_id, Stage::Signature).unwrap();
        let detail = get_case(&db, &notary, &case_id).unwrap();
        assert_eq!(detail.summary.stage, "signature");
        assert_eq!(detail.stages.len(), 7);
    }

    #[test]
    fn confidential_cases_are_hidden_from_unnamed_users() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(dir.path());
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        let clerk = Principal::new("u2", vec![Role::Clerk]);
        let case_id = create_case(&db, &notary, "sale", "u1", true, None, None).unwrap();

        assert!(matches!(
            get_case(&db, &clerk, &case_id),
            Err(CoreError::Denied)
        ));
        assert!(get_case(&db, &notary, &case_id).is_ok());

        // Granting access through the ACL is enough for the named clerk.
        db.connection()
            .execute(
                "INSERT INTO case_acl (case_id, user_id, granted_at_ms) VALUES (?1, 'u2', 0)",
                rusqlite::params![case_id],
            )
            .unwrap();
        assert!(get_case(&db, &clerk, &case_id).is_ok());
    }

    #[test]
    fn deadlines_are_reported_for_the_home_card() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(dir.path());
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        let now = crate::now_ms();
        create_case(&db, &notary, "sale", "u1", false, Some(now + 3 * 86_400_000), None).unwrap();
        create_case(&db, &notary, "sale", "u1", false, Some(now + 40 * 86_400_000), None).unwrap();

        let soon = approaching_deadlines(&db, now, 7).unwrap();
        assert_eq!(soon.len(), 1);
    }

    #[test]
    fn listing_filters_confidential_cases_by_acl() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(dir.path());
        let notary = Principal::new("u1", vec![Role::NotaryTitular]);
        let clerk = Principal::new("u2", vec![Role::Clerk]);
        create_case(&db, &notary, "sale", "u1", false, None, None).unwrap();
        create_case(&db, &notary, "succession", "u1", true, None, None).unwrap();

        assert_eq!(list_cases(&db, &notary, 50).unwrap().len(), 2);
        let clerk_view = list_cases(&db, &clerk, 50).unwrap();
        assert_eq!(clerk_view.len(), 1);
        assert!(!clerk_view[0].confidential);
    }

    #[test]
    fn a_clerk_without_case_create_permission_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(dir.path());
        let inspector = Principal::new("u9", vec![Role::Inspector]);
        assert!(matches!(
            create_case(&db, &inspector, "sale", "u9", false, None, None),
            Err(CoreError::Denied)
        ));
    }
}
