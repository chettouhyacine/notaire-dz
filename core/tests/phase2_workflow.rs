//! End-to-end Phase 2 workflow: a clerk opens a client, the notary opens a case with a checklist,
//! a document is stored and read back, the act is signed, numbered in the register, and a copy is
//! issued. Every step must leave an audit trace and must be unforgeable afterwards.

use notaire_core::casefile::{self, ChecklistSeedItem, ChecklistTemplateSpec, Stage};
use notaire_core::crypto::{KdfParams, SecretKey};
use notaire_core::party::{self, NameForm, PartyInput, PersonInput, Script};
use notaire_core::rbac::{Principal, Role};
use notaire_core::registers;
use notaire_core::storage::Database;
use notaire_core::vault::{self, StoreDocumentInput, Vault};

fn seed(db: &Database) {
    let hash = notaire_core::auth::hash_password("Correct-Horse-9", KdfParams::for_tests()).unwrap();
    db.create_user(
        "u-notary",
        "notaire",
        Some("Maître FICTIF"),
        &hash,
        &[Role::NotaryTitular.code()],
        false,
    )
    .unwrap();
    db.create_user(
        "u-clerk",
        "clerc",
        Some("Clerc FICTIF"),
        &hash,
        &[Role::Clerk.code()],
        false,
    )
    .unwrap();
}

fn sale_checklist() -> ChecklistTemplateSpec {
    ChecklistTemplateSpec {
        act_type: "sale".into(),
        version_no: 1,
        source: "DEMO ONLY — [VERIFY]".into(),
        status: "verify".into(),
        items: vec![
            ChecklistSeedItem {
                code: "title_deed".into(),
                label_key: "checklist.sale.title".into(),
                required: true,
                ordinal: 1,
            },
            ChecklistSeedItem {
                code: "mortgage_certificate".into(),
                label_key: "checklist.sale.mortgage".into(),
                required: true,
                ordinal: 2,
            },
            ChecklistSeedItem {
                code: "bank_proof".into(),
                label_key: "checklist.sale.bank_proof".into(),
                required: true,
                ordinal: 3,
            },
        ],
    }
}

#[test]
fn clerk_to_numbered_act_to_issued_copy() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path().join("office.sqlite"), SecretKey::from_bytes([21u8; 32]))
        .unwrap();
    seed(&db);
    let vault = Vault::new(dir.path().join("vault")).unwrap();

    let clerk = Principal::new("u-clerk", vec![Role::Clerk]);
    let notary = Principal::new("u-notary", vec![Role::NotaryTitular]);

    // 1. The clerk creates the client.
    let seller = party::create_party(
        &db,
        &clerk.user_id,
        &PartyInput::Person(PersonInput {
            names: vec![
                NameForm {
                    script: Script::Arabic,
                    surname: "بن زاكور".into(),
                    given: "محمد".into(),
                },
                NameForm {
                    script: Script::Latin,
                    surname: "BENZAKOUR".into(),
                    given: "Mohamed".into(),
                },
            ],
            birth_date: Some("1975-06-30".into()),
            nin: Some("109876543210987654".into()),
            ..Default::default()
        }),
    )
    .unwrap();

    assert!(!seller.is_empty());

    // Duplicate detection warns before a second record is created.
    let candidate = party::duplicate_candidates(
        &db,
        &PartyInput::Person(PersonInput {
            names: vec![NameForm {
                script: Script::Latin,
                surname: "BENZAKOUR".into(),
                given: "Mohamed".into(),
            }],
            ..Default::default()
        }),
        10,
    )
    .unwrap();
    assert_eq!(candidate.len(), 1);
    assert!(candidate[0].exact);

    // 2. The notary opens the case with its checklist.
    let case_id = casefile::create_case(
        &db,
        &notary,
        "sale",
        "u-clerk",
        true, // confidential: a client matter
        Some(notaire_core::now_ms() + 20 * 86_400_000),
        Some(&sale_checklist()),
    )
    .unwrap();

    // The clerk is not on the ACL of a confidential case until explicitly added.
    assert!(matches!(
        casefile::get_case(&db, &clerk, &case_id),
        Err(notaire_core::CoreError::Denied)
    ));
    db.connection()
        .execute(
            "INSERT INTO case_acl (case_id, user_id, granted_at_ms) VALUES (?1, 'u-clerk', 0)",
            rusqlite::params![case_id],
        )
        .unwrap();

    let opened = casefile::get_case(&db, &clerk, &case_id).unwrap();
    assert_eq!(opened.missing_documents.len(), 3);

    // 3. A document arrives, is stored encrypted, and is read back with its hash checked.
    let record = vault::store_document(
        &db,
        &vault,
        &clerk,
        StoreDocumentInput {
            case_id: Some(&case_id),
            title: "Acte de propriété fictif",
            mime: "application/pdf",
            bytes: b"%PDF-1.7 acte fictif - BENZAKOUR",
            retention_class: "case",
        },
    )
    .unwrap();
    let (_, bytes) = vault::read_document(&db, &vault, &clerk, &record.id).unwrap();
    assert!(bytes.starts_with(b"%PDF-1.7"));

    // OCR output is stored encrypted and marked as requiring confirmation.
    vault::attach_ocr_text(&db, &clerk, &record.id, "رقم ٢٠٢٦ ملكية").unwrap();
    assert_eq!(
        vault::ocr_text(&db, &clerk, &record.id).unwrap().as_deref(),
        Some("رقم ٢٠٢٦ ملكية")
    );

    // 4. The file advances to signature; the clerk cannot take that step.
    for stage in [
        Stage::Documents,
        Stage::Verification,
        Stage::Drafting,
        Stage::Review,
        Stage::Reading,
    ] {
        casefile::advance_stage(&db, &notary, &case_id, stage).unwrap();
    }
    assert!(matches!(
        casefile::advance_stage(&db, &clerk, &case_id, Stage::Signature),
        Err(notaire_core::CoreError::Denied)
    ));
    casefile::advance_stage(&db, &notary, &case_id, Stage::Signature).unwrap();

    // 5. Numbering: gap-free, chained, and refused to the clerk.
    let register = registers::ensure_register(&db, "office-1", "repertoire", "R", 2026).unwrap();
    assert!(matches!(
        registers::allocate_number(&db, &clerk, &register, Some(&case_id), "2026/0001"),
        Err(notaire_core::CoreError::Denied)
    ));
    let entry = registers::allocate_number(&db, &notary, &register, Some(&case_id), "2026/0001")
        .unwrap();
    assert_eq!(entry.seq, 1);
    registers::verify_register(&db, &register).unwrap();

    // 6. Copies: numbered per kind, watermarked, immutable.
    let copy = registers::issue_copy(&db, &clerk, "2026/0001", Some(&case_id), "authentic", "M. FICTIF")
        .unwrap();
    assert_eq!(copy.copy_number, 1);
    assert!(copy.watermark.contains("2026/0001"));

    // 7. The whole session is on the audit chain, and the chain verifies.
    let actions: Vec<String> = db
        .audit_events()
        .unwrap()
        .into_iter()
        .map(|e| e.action)
        .collect();
    for expected in [
        "party.create",
        "case.create",
        "document.store",
        "document.read",
        "document.ocr",
        "case.stage",
        "register.allocate",
        "copy.issue",
    ] {
        assert!(
            actions.iter().any(|a| a == expected),
            "missing audit action {expected}"
        );
    }
    db.verify_audit().unwrap();

    // 8. Nothing sensitive is readable on disk, in the database or the vault.
    let needles: [&[u8]; 2] = [b"BENZAKOUR", b"109876543210987654"];
    for path in collect_files(dir.path()) {
        let bytes = std::fs::read(&path).unwrap();
        for needle in needles {
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle),
                "plaintext {:?} found in {}",
                String::from_utf8_lossy(needle),
                path.display()
            );
        }
    }
}

fn collect_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_files(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}
