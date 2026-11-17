//! Storage-level security tests.
//!
//! These assert properties a notary's data must have on disk, not implementation details:
//! no readable client data, an append-only audit trail that survives reopen, migrations that refuse
//! to run against a tampered schema record, and rules that reproduce their figures after reload.

use notaire_core::crypto::{KdfParams, SecretKey};
use notaire_core::date::Date;
use notaire_core::money::{Money, Rounding};
use notaire_core::rbac::Role;
use notaire_core::rules::{ReviewStatus, Rule, RuleKind, RulePayload, RuleSet, RuleSetKind};
use notaire_core::storage::Database;

fn master() -> SecretKey {
    SecretKey::from_bytes([3u8; 32])
}

fn seed_notary(db: &Database) {
    let hash = notaire_core::auth::hash_password(
        "Correct-Horse-9",
        KdfParams::for_tests(),
    )
    .unwrap();
    db.create_user(
        "u-notary",
        "notaire",
        Some("Maître FICTIF Ahmed"),
        &hash,
        &[Role::NotaryTitular.code()],
        false,
    )
    .unwrap();
}

#[test]
fn sensitive_values_never_appear_in_plaintext_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("office.sqlite");
    let db = Database::open(&db_path, master()).unwrap();
    seed_notary(&db);

    // A fictitious national identity number and a family name, as they would be stored.
    let nin = "109876543210987654";
    let name = "BENZAKOUR-FICTIF";
    let enc = db.encrypt_field(nin, "party:p1:nin").unwrap();
    db.connection()
        .execute(
            "INSERT INTO app_setting (key, value, updated_at_ms) VALUES ('party.p1.nin', ?, 0)",
            rusqlite::params![enc],
        )
        .unwrap();
    db.connection()
        .execute(
            "INSERT INTO app_setting (key, value, updated_at_ms) VALUES ('party.p1.name', ?, 0)",
            rusqlite::params![db.encrypt_field(name, "party:p1:name").unwrap()],
        )
        .unwrap();

    // Also store a blind index, which is what search actually uses.
    let bidx = db.blind_index(name);
    db.connection()
        .execute(
            "INSERT INTO app_setting (key, value, updated_at_ms) VALUES ('party.p1.name.bidx', ?, 0)",
            rusqlite::params![bidx.as_slice()],
        )
        .unwrap();

    // Flush the WAL so the on-disk bytes are complete, then scan every file of the database.
    db.connection().execute_batch("PRAGMA wal_checkpoint(FULL);").unwrap();

    let mut checked = 0;
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        checked += 1;
        for needle in [nin.as_bytes(), name.as_bytes()] {
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle),
                "plaintext {:?} found in {}",
                String::from_utf8_lossy(needle),
                path.display()
            );
        }
    }
    assert!(checked >= 1, "expected at least the database file to be scanned");

    // And the value is still recoverable with the key, so the test is not vacuous.
    let stored: Vec<u8> = db
        .connection()
        .query_row(
            "SELECT value FROM app_setting WHERE key = 'party.p1.nin'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(db.decrypt_field(&stored, "party:p1:nin").unwrap(), nin);
    assert_eq!(
        db.blind_index(name),
        db.blind_index("benzakour fictif").to_owned(),
        "blind index must be stable and normalised"
    );
}

#[test]
fn audit_chain_survives_reopen_and_stays_append_only() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("office.sqlite");

    {
        let db = Database::open(&db_path, master()).unwrap();
        seed_notary(&db);
        db.append_audit("u-notary", "case.read", "case-1", "reason=consultation")
            .unwrap();
        db.append_audit("u-notary", "export.create", "case-1", "format=pdf")
            .unwrap();
        db.verify_audit().unwrap();
    }

    // Reopen: the key material is unwrapped from the database, the chain must continue seamlessly.
    let db = Database::open(&db_path, master()).unwrap();
    db.verify_audit().unwrap();
    db.append_audit("u-notary", "auth.logout", "u-notary", "").unwrap();
    db.verify_audit().unwrap();

    let events = db.audit_events().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[1].prev_hash, events[0].hash);

    // The database itself refuses to rewrite history.
    let update = db
        .connection()
        .execute("UPDATE audit_event SET detail = 'x' WHERE seq = 1", []);
    assert!(update.is_err(), "audit rows must not be updatable");

    let delete = db
        .connection()
        .execute("DELETE FROM audit_event WHERE seq = 1", []);
    assert!(delete.is_err(), "audit rows must not be deletable");
}

#[test]
fn a_wrong_master_key_cannot_read_the_office_data() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("office.sqlite");
    {
        let db = Database::open(&db_path, master()).unwrap();
        seed_notary(&db);
    }
    // The wrapped office keys cannot be unwrapped with the wrong master key, so opening the office
    // fails outright rather than exposing anything.
    match Database::open(&db_path, SecretKey::from_bytes([4u8; 32])) {
        Err(notaire_core::CoreError::Crypto(_)) => {}
        Err(other) => panic!("wrong key must fail as a crypto error, got {other:?}"),
        Ok(_) => panic!("a wrong master key must not open the office"),
    }
}

#[test]
fn a_tampered_migration_checksum_aborts_startup() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("office.sqlite");
    {
        let db = Database::open(&db_path, master()).unwrap();
        db.connection()
            .execute(
                "UPDATE schema_migration SET checksum = 'deadbeef' WHERE id = '0001_init'",
                [],
            )
            .unwrap();
    }
    let reopened = Database::open(&db_path, master());
    assert!(reopened.is_err(), "a changed migration must not be silently repaired");
}

#[test]
fn rules_persist_and_reproduce_the_same_calculation() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("office.sqlite");
    let db = Database::open(&db_path, master()).unwrap();

    let set = RuleSet {
        id: "rs-demo-2024".into(),
        code: "SALE_COSTS_DEMO".into(),
        kind: RuleSetKind::Fee,
        version_no: 1,
        valid_from: Date::from_ymd(2024, 1, 1).unwrap(),
        valid_to: None,
        source: "DEMO ONLY".into(),
        status: ReviewStatus::Verif,
        reviewed_by: None,
        reviewed_on: None,
    };
    db.insert_rule_set(&set).unwrap();
    for (i, (code, from, to, num, den, applies)) in [
        ("T1", 0i64, Some(10_000_000i64), 5i64, 100i64, "notary_fee"),
        ("T2", 10_000_000, None, 2, 100, "notary_fee"),
    ]
    .iter()
    .enumerate()
    {
        db.insert_rule(&Rule {
            id: format!("r{i}"),
            rule_set_id: set.id.clone(),
            code: (*code).into(),
            kind: RuleKind::Tranche,
            label_key: format!("calc.line.{code}"),
            priority: i as i32,
            payload: RulePayload::Tranche {
                from_cents: *from,
                to_cents: *to,
                rate_num: *num,
                rate_den: *den,
                min_cents: None,
                max_cents: None,
            },
            status: ReviewStatus::Verif,
            applies_to: (*applies).into(),
        })
        .unwrap();
    }

    let engine = db.load_rules_engine().unwrap();
    assert!(engine.validate_no_overlaps().is_ok());
    let base = Money::parse("120000.00").unwrap();
    let calc = engine
        .calculate(
            "SALE_COSTS_DEMO",
            base,
            Date::from_ymd(2024, 6, 1).unwrap(),
            Rounding::HalfUp,
        )
        .unwrap();
    assert_eq!(calc.notary_fees_cents, 540_000);
    assert!(calc.contains_unverified_rules, "[VERIFY] must survive the round trip");

    db.store_calculation("calc-1", &calc, "u-notary").unwrap();

    // A stored calculation is a receipt: it cannot be edited…
    assert!(db
        .connection()
        .execute("UPDATE calculation SET total_cents = 1 WHERE id = 'calc-1'", [])
        .is_err());

    // …and it reloads exactly.
    let reloaded = db.load_calculation("calc-1").unwrap().unwrap();
    assert_eq!(reloaded.total_cents, calc.total_cents);
    assert_eq!(reloaded.lines, calc.lines);
    assert_eq!(reloaded.rule_set_ids, calc.rule_set_ids);

    // Rule sets are closed with valid_to, never deleted.
    assert!(db
        .connection()
        .execute("DELETE FROM rule_set WHERE id = 'rs-demo-2024'", [])
        .is_err());
}

#[test]
fn login_locks_out_after_repeated_failures() {
    use notaire_core::auth::{AuthConfig, AuthService};

    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path().join("office.sqlite"), master()).unwrap();
    seed_notary(&db);
    let auth = AuthService::new(&db, AuthConfig::default());

    let now = 1_700_000_000_000;
    for _ in 0..4 {
        assert!(auth.login("notaire", "Wrong-Password-1", now).is_err());
    }
    // Fifth failure locks the account.
    assert!(auth.login("notaire", "Wrong-Password-1", now).is_err());
    match auth.login("notaire", "Correct-Horse-9", now) {
        Err(notaire_core::CoreError::LockedOut { until_ms }) => assert!(until_ms > now),
        other => panic!("expected lockout, got {other:?}"),
    }
    // After the lockout window the correct password works again.
    let later = now + 6 * 60 * 1000; // past the lockout window
    let session = auth.login("notaire", "Correct-Horse-9", later).unwrap();
    assert!(session.principal().is_notary());
    auth.require_fresh(&session, later).unwrap();
    auth.require_fresh(&session, later + 10 * 60 * 1000)
        .expect_err("stale session must re-authenticate");

    // Every outcome is in the chain.
    let actions: Vec<String> = db
        .audit_events()
        .unwrap()
        .into_iter()
        .map(|e| e.action)
        .collect();
    assert!(actions.iter().filter(|a| *a == "auth.login_failed").count() >= 5);
    assert!(actions.iter().any(|a| a == "auth.login"));
}
