//! Desktop shell command API.
//!
//! The UI never touches the database or the filesystem: every action is a command that runs
//! through `notaire-core`, where permissions and the audit chain are enforced.
//!
//! Phase 1 scope, stated plainly: one office passphrase both unlocks the encrypted office and
//! authenticates the notary. Per-user key wrapping (so a clerk can log in without holding the
//! office key) is a Phase 2 item; until then this build is a single-user foundation.

use std::path::PathBuf;
use std::sync::Mutex;

use notaire_core::audit::AuditLog;
use notaire_core::auth::{AuthConfig, AuthService, Session};
use notaire_core::backup::{self, BackupInfo};
use notaire_core::crypto::{derive_master_key, KdfParams};
use notaire_core::rbac::Role;
use notaire_core::recovery;
use notaire_core::storage::{self, Database};
use notaire_core::{CoreError, OfficePaths};
use serde::{Deserialize, Serialize};
use tauri::State;

struct Unlocked {
    db: Database,
    session: Session,
}

#[derive(Default)]
struct AppState {
    unlocked: Mutex<Option<Unlocked>>,
}

#[derive(Debug, Serialize)]
struct OfficeStatus {
    initialized: bool,
    data_dir: String,
    app_version: String,
    /// Whether this office has a recovery code, i.e. whether a forgotten passphrase can be reset.
    recovery_available: bool,
    /// True only when the shell was started with `--demo` / `NOTAIRE_DZ_DEMO=1`.
    demo: bool,
}

/// Demo mode is opt-in per launch and never stored: without the flag nothing changes.
///
/// It exists so the interface can be reviewed without a passphrase. It auto-creates a throwaway
/// office whose passphrase is written to `demo-key.txt` in the data folder — which is exactly why it
/// must never be used for a real office, and why the UI shows a permanent demo badge.
fn demo_enabled() -> bool {
    let from_env = std::env::var("NOTAIRE_DZ_DEMO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    from_env || std::env::args().any(|arg| arg == "--demo")
}

/// A separate folder for demo data, so a demo launch can never land in the real office.
fn demo_paths() -> notaire_core::Result<notaire_core::OfficePaths> {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("NotaireDZ-demo");
    let paths = notaire_core::OfficePaths::new(base);
    paths.ensure()?;
    Ok(paths)
}

fn demo_passphrase() -> notaire_core::Result<String> {
    let salt = notaire_core::crypto::random_salt()?;
    Ok(format!("{}9x", hex::encode(salt)))
}

/// The device key that opens the office, kept beside the database (`office.key`).
///
/// Data at rest is still encrypted; what changes is who can open it. With a key file the sign-in
/// credential is a screen lock (admin/admin by default), so a forgotten password is no longer a
/// locked office. Setups that already have a passphrase-derived office keep working unchanged.
const DEVICE_KEY_FILE: &str = "office.key";

fn device_key(paths: &notaire_core::OfficePaths) -> Option<String> {
    std::fs::read_to_string(paths.root.join(DEVICE_KEY_FILE))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// First run: create the office with a device key and the documented `admin` account, so the app
/// opens on a prefilled sign-in form instead of a setup wizard.
fn provision_default(
    state: &AppState,
    paths: &notaire_core::OfficePaths,
) -> notaire_core::Result<InitResult> {
    let key = demo_passphrase()?; // 32 random hex characters plus a suffix: strong, not a credential
    std::fs::write(paths.root.join(DEVICE_KEY_FILE), &key)?;
    let created = bootstrap_office(
        state,
        paths,
        "مكتب التوثيق",
        "الموثق",
        "admin",
        &key,
        "admin",
    )?;
    // Provisioning must not sign anyone in: the app starts at the sign-in screen.
    *state.unlocked.lock().unwrap() = None;
    Ok(created)
}

#[derive(Debug, Serialize)]
struct InitResult {
    principal: PrincipalDto,
    /// Shown once, at setup: the only way back into the office if the passphrase is forgotten.
    recovery_code: String,
}

#[derive(Debug, Serialize)]
struct ResetResult {
    /// A fresh code replaces the one just used, which is now worthless.
    recovery_code: String,
}

#[derive(Debug, Deserialize)]
struct InitPayload {
    office_name: String,
    notary_display_name: String,
    username: String,
    passphrase: String,
}

#[derive(Debug, Serialize)]
struct PrincipalDto {
    username: String,
    roles: Vec<String>,
    display_name: String,
}

#[derive(Debug, Serialize)]
struct AuditReport {
    head_hex: String,
    events: u64,
    chain_ok: bool,
}

fn data_dir() -> notaire_core::Result<OfficePaths> {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("NotaireDZ");
    let paths = OfficePaths::new(base);
    paths.ensure()?;
    Ok(paths)
}

fn error_json(error: CoreError) -> notaire_core::UiError {
    error.into()
}

#[tauri::command]
fn office_status(state: State<'_, AppState>) -> Result<OfficeStatus, notaire_core::UiError> {
    let paths = data_dir().map_err(error_json)?;
    // A fresh install provisions itself: device key + `admin` account, no wizard in the way.
    if !paths.database().exists() && !demo_enabled() {
        provision_default(state.inner(), &paths).map_err(error_json)?;
    }
    let initialized = paths.database().exists();
    let unlocked = state.unlocked.lock().map_err(|_| notaire_core::UiError {
        code: "state".into(),
        message_key: "error.storage".into(),
        detail: None,
    })?;
    Ok(OfficeStatus {
        initialized: initialized && unlocked.is_some() || initialized,
        data_dir: paths.root.display().to_string(),
        app_version: notaire_core::APP_VERSION.to_string(),
        recovery_available: initialized && recovery::read_escrow_file(&paths.database())?.is_some(),
        demo: demo_enabled(),
    })
}

/// Create the office on disk and leave the caller signed in.
///
/// Shared by first-run setup and demo mode so the two can never drift apart: same encryption
/// envelope, same recovery code, same audit entries.
fn bootstrap_office(
    state: &AppState,
    paths: &notaire_core::OfficePaths,
    office_name: &str,
    notary_name: &str,
    username: &str,
    passphrase: &str,
    credential: &str,
) -> notaire_core::Result<InitResult> {
    let params = KdfParams::standard();
    let salt = storage::create_salt_file(paths.salt())?;
    let master = derive_master_key(passphrase, &salt, params)?;
    let db = Database::open(paths.database(), master.clone())?;

    let office_id = "office-1";
    db.insert_office(office_id, office_name)?;

    // A credential equal to the passphrase is the classic path and must satisfy the strength policy;
    // a separate credential belongs to a device-key office and is a screen lock only.
    let hash = if credential == passphrase {
        notaire_core::auth::hash_password(credential, params)?
    } else {
        notaire_core::auth::hash_password_lenient(credential, params)?
    };
    db.create_user("user-notary-1", username, Some(notary_name), &hash, &["notary_titular"], false)?;

    // Two envelopes around the same master key: one for the passphrase (so it can be changed
    // later) and one for the recovery code (so a forgotten passphrase is not fatal).
    let recovery_code = recovery::generate_code()?;
    recovery::store_escrow(db.connection(), &recovery::seal_master(&recovery_code, &master)?)?;
    let wrapped = recovery::wrap_master_for_passphrase(passphrase, &salt, params, &master)?;
    recovery::store_passphrase_wrap(db.connection(), "user-notary-1", &salt, &wrapped, params)?;

    db.append_audit("user-notary-1", "office.init", office_id, "phase=1")?;
    db.append_audit("user-notary-1", "recovery.create", office_id, "escrow=created")?;

    let session = AuthService::new(&db, AuthConfig::default()).login(
        username,
        credential,
        notaire_core::now_ms(),
    )?;
    let principal = PrincipalDto {
        username: session.username.clone(),
        roles: session.roles.iter().map(|r| r.code().to_string()).collect(),
        display_name: notary_name.to_string(),
    };
    *state.unlocked.lock().unwrap() = Some(Unlocked { db, session });
    Ok(InitResult { principal, recovery_code })
}

#[tauri::command]
fn office_initialize(
    state: State<'_, AppState>,
    payload: InitPayload,
) -> Result<InitResult, notaire_core::UiError> {
    (|| -> notaire_core::Result<InitResult> {
        let paths = data_dir()?;
        if paths.database().exists() {
            return Err(CoreError::Validation("office is already initialised".into()));
        }
        if payload.office_name.trim().is_empty() {
            return Err(CoreError::Validation("office name is required".into()));
        }
        bootstrap_office(
            state.inner(),
            &paths,
            &payload.office_name,
            &payload.notary_display_name,
            &payload.username,
            &payload.passphrase,
            &payload.passphrase,
        )
    })()
    .map_err(error_json)
}

/// Demo launch: skip the sign-in screens.
///
/// Creates a throwaway office on first use and remembers its random passphrase in `demo-key.txt`, so
/// later demo launches reopen the same data. Refuses to run without the launch flag, so the real
/// office can never be opened this way, and a demo office that happens to exist without its key file
/// is reported rather than silently replaced.
#[tauri::command]
fn demo_sign_in(state: State<'_, AppState>) -> Result<PrincipalDto, notaire_core::UiError> {
    (|| -> notaire_core::Result<PrincipalDto> {
        if !demo_enabled() {
            return Err(CoreError::Denied);
        }
        let paths = demo_paths()?;
        let key_path = paths.root.join("demo-key.txt");
        if !paths.database().exists() {
            let passphrase = demo_passphrase()?;
            std::fs::write(&key_path, &passphrase)?;
            let created = bootstrap_office(
                state.inner(),
                &paths,
                "مكتب تجريبي",
                "الموثق التجريبي",
                "notaire",
                &passphrase,
                &passphrase,
            )?;
            return Ok(created.principal);
        }

        let passphrase = std::fs::read_to_string(&key_path).map_err(|_| {
            CoreError::Validation(
                "a demo office exists but its key file is missing; sign in normally instead".into(),
            )
        })?;
        let passphrase = passphrase.trim().to_string();
        let db = open_office(&paths, "notaire", &passphrase)?;
        db.verify_audit()?;
        let auth = AuthService::new(&db, AuthConfig::default());
        let session = auth.login("notaire", &passphrase, notaire_core::now_ms())?;
        let principal = PrincipalDto {
            username: session.username.clone(),
            roles: session.roles.iter().map(|r| r.code().to_string()).collect(),
            display_name: "الموثق التجريبي".to_string(),
        };
        *state.unlocked.lock().unwrap() = Some(Unlocked { db, session });
        Ok(principal)
    })()
    .map_err(error_json)
}

/// Open the office file for sign-in.
///
/// Normally the master key is unwrapped through the user's passphrase wrap, which keeps the master
/// key stable across a passphrase change. An office created before wraps were written falls back to
/// deriving the master key from the passphrase directly.
fn open_office(
    paths: &notaire_core::OfficePaths,
    username: &str,
    passphrase: &str,
) -> notaire_core::Result<Database> {
    let path = paths.database();
    // Device-key offices open without the credential; the credential is verified against its hash
    // afterwards by `AuthService::login`.
    if let Some(key) = device_key(paths) {
        let salt = storage::read_salt_file(paths.salt())?;
        let master = derive_master_key(&key, &salt, KdfParams::standard())?;
        return Database::open(&path, master);
    }
    if let Some(wrap) = recovery::read_wrap_file(&path, username)? {
        let master = recovery::unwrap_master_with_passphrase(
            passphrase,
            &wrap.salt,
            wrap.params,
            &wrap.wrapped,
        )
        .map_err(|_| CoreError::Denied)?;
        return Database::open(&path, master);
    }
    let salt = storage::read_salt_file(paths.salt())?;
    let master = derive_master_key(passphrase, &salt, KdfParams::standard())
        .map_err(|_| CoreError::Denied)?;
    Database::open(&path, master)
}

#[tauri::command]
fn auth_login(
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> Result<PrincipalDto, notaire_core::UiError> {
    (|| -> notaire_core::Result<PrincipalDto> {
        let paths = data_dir()?;
        let db = open_office(&paths, &username, &password)?;

        // The chain must be intact before anyone works on the data.
        db.verify_audit()?;

        let auth = AuthService::new(&db, AuthConfig::default());
        let session = auth.login(&username, &password, notaire_core::now_ms())?;
        let principal = PrincipalDto {
            username: session.username.clone(),
            roles: session.roles.iter().map(|r| r.code().to_string()).collect(),
            display_name: session.username.clone(),
        };
        *state.unlocked.lock().unwrap() = Some(Unlocked { db, session });
        Ok(principal)
    })()
    .map_err(error_json)
}

/// Reset a forgotten passphrase with the recovery code issued at setup.
///
/// No session is needed: this runs before sign-in. The master key is unwrapped from the escrow, so
/// the data is untouched, and both the password hash and the passphrase wrap are replaced.
#[tauri::command]
fn auth_reset_passphrase(
    username: String,
    recovery_code: String,
    new_passphrase: String,
) -> Result<ResetResult, notaire_core::UiError> {
    (|| -> notaire_core::Result<ResetResult> {
        let paths = data_dir()?;
        let Some(escrow) = recovery::read_escrow_file(&paths.database())? else {
            return Err(CoreError::Validation(
                "this office has no recovery code registered".into(),
            ));
        };
        let master = recovery::open_master(&recovery_code, &escrow)
            .map_err(|_| CoreError::Validation("recovery code is incorrect".into()))?;
        let db = Database::open(paths.database(), master.clone())?;
        db.verify_audit()?;

        recovery::reset_user_passphrase(&db, &username, &new_passphrase)?;

        let params = KdfParams::standard();
        let salt = recovery::new_wrap_salt()?;
        let wrapped = recovery::wrap_master_for_passphrase(&new_passphrase, &salt, params, &master)?;
        let user = db.find_user(&username)?.ok_or(CoreError::Denied)?;
        recovery::store_passphrase_wrap(db.connection(), &user.id, &salt, &wrapped, params)?;

        let recovery_code = recovery::reissue_escrow(&db, &master)?;
        Ok(ResetResult { recovery_code })
    })()
    .map_err(error_json)
}

/// Change the account's username. The session stays open; only the label changes.
#[tauri::command]
fn account_rename(
    state: State<'_, AppState>,
    new_username: String,
) -> Result<PrincipalDto, notaire_core::UiError> {
    (|| -> notaire_core::Result<PrincipalDto> {
        let mut guard = state.unlocked.lock().unwrap();
        let unlocked = guard.as_mut().ok_or(CoreError::ReauthRequired)?;
        notaire_core::auth::rename_user(
            &unlocked.db,
            &unlocked.session.user_id,
            &new_username,
        )?;
        unlocked.session.username = new_username.trim().to_string();
        Ok(PrincipalDto {
            username: unlocked.session.username.clone(),
            roles: unlocked.session.roles.iter().map(|r| r.code().to_string()).collect(),
            display_name: unlocked.session.username.clone(),
        })
    })()
    .map_err(error_json)
}

/// Change the account's password (the office passphrase): requires the current one.
#[tauri::command]
fn account_change_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), notaire_core::UiError> {
    let uses_device_key = device_key(&data_dir().map_err(error_json)?).is_some();
    with_session(&state, |db, session| {
        if uses_device_key {
            // The data key does not come from this credential, so no re-wrapping and no strength rule.
            recovery::change_credential_only(db, &session.user_id, &current_password, &new_password)
        } else {
            recovery::change_password(db, &session.user_id, &current_password, &new_password)
        }
    })
}

#[tauri::command]
fn auth_logout(state: State<'_, AppState>) -> Result<(), notaire_core::UiError> {
    let mut guard = state.unlocked.lock().unwrap();
    if let Some(unlocked) = guard.as_ref() {
        let auth = AuthService::new(&unlocked.db, AuthConfig::default());
        let _ = auth.logout(&unlocked.session);
    }
    *guard = None;
    Ok(())
}

#[tauri::command]
fn audit_head(state: State<'_, AppState>) -> Result<AuditReport, notaire_core::UiError> {
    (|| -> notaire_core::Result<AuditReport> {
        let guard = state.unlocked.lock().unwrap();
        let unlocked = guard.as_ref().ok_or(CoreError::ReauthRequired)?;
        let events = unlocked.db.audit_events()?;
        let ok = unlocked.db.verify_audit().is_ok();
        Ok(AuditReport {
            head_hex: AuditLog::head_hash_hex(&events),
            events: events.len() as u64,
            chain_ok: ok,
        })
    })()
    .map_err(error_json)
}

#[tauri::command]
fn backup_create(
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<BackupInfo, notaire_core::UiError> {
    (|| -> notaire_core::Result<BackupInfo> {
        let paths = data_dir()?;
        let salt = storage::read_salt_file(paths.salt())?;
        let key = backup::derive_backup_key(&passphrase, &salt, KdfParams::standard())?;
        let guard = state.unlocked.lock().unwrap();
        let unlocked = guard.as_ref().ok_or(CoreError::ReauthRequired)?;

        let stamp = notaire_core::now_ms();
        let dest = paths.backups().join(format!("office-{stamp}.nzdzbak"));
        let info = backup::create_backup(&unlocked.db, &dest, &key)?;
        unlocked
            .db
            .append_audit(&unlocked.session.user_id, "backup.create", &dest.display().to_string(), "")?;
        Ok(info)
    })()
    .map_err(error_json)
}

#[tauri::command]
fn backup_verify(
    path: String,
    passphrase: String,
) -> Result<BackupInfo, notaire_core::UiError> {
    (|| -> notaire_core::Result<BackupInfo> {
        let paths = data_dir()?;
        let salt = storage::read_salt_file(paths.salt())?;
        let key = backup::derive_backup_key(&passphrase, &salt, KdfParams::standard())?;
        backup::verify_backup(path, &key)
    })()
    .map_err(error_json)
}

#[derive(Debug, Deserialize)]
struct PersonPayload {
    surname_ar: String,
    given_ar: String,
    surname_latin: String,
    given_latin: String,
    father_name: Option<String>,
    mother_name: Option<String>,
    birth_date: Option<String>,
    nin: Option<String>,
}

#[derive(Debug, Serialize)]
struct PartyDto {
    id: String,
    kind: String,
    display_ar: String,
    display_latin: String,
}

#[derive(Debug, Deserialize)]
struct CasePayload {
    act_type: String,
    owner_user_id: String,
    due_in_days: Option<i64>,
    confidential: bool,
}

#[derive(Debug, Serialize)]
struct CaseDto {
    id: String,
    reference: String,
    act_type: String,
    stage: String,
    opened_at_ms: i64,
    due_at_ms: Option<i64>,
    confidential: bool,
}

fn with_session<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&Database, &Session) -> notaire_core::Result<T>,
) -> Result<T, notaire_core::UiError> {
    (|| {
        let guard = state.unlocked.lock().unwrap();
        let unlocked = guard.as_ref().ok_or(CoreError::ReauthRequired)?;
        f(&unlocked.db, &unlocked.session)
    })()
    .map_err(error_json)
}

#[tauri::command]
fn party_create(
    state: State<'_, AppState>,
    payload: PersonPayload,
) -> Result<String, notaire_core::UiError> {
    with_session(&state, |db, session| {
        let principal = session.principal();
        let mut names = Vec::new();
        if !payload.surname_ar.trim().is_empty() && !payload.given_ar.trim().is_empty() {
            names.push(notaire_core::party::NameForm {
                script: notaire_core::party::Script::Arabic,
                surname: payload.surname_ar.clone(),
                given: payload.given_ar.clone(),
            });
        }
        names.push(notaire_core::party::NameForm {
            script: notaire_core::party::Script::Latin,
            surname: payload.surname_latin.clone(),
            given: payload.given_latin.clone(),
        });
        notaire_core::party::create_party(
            db,
            &principal.user_id,
            &notaire_core::party::PartyInput::Person(notaire_core::party::PersonInput {
                names,
                father_name: payload.father_name,
                mother_name: payload.mother_name,
                birth_date: payload.birth_date,
                nin: payload.nin,
                ..Default::default()
            }),
        )
    })
}

#[tauri::command]
fn party_list(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<PartyDto>, notaire_core::UiError> {
    with_session(&state, |db, session| {
        let principal = session.principal();
        Ok(notaire_core::party::list_parties(db, &principal, limit.unwrap_or(100))?
            .into_iter()
            .map(|p| PartyDto {
                id: p.id,
                kind: p.kind,
                display_ar: p.display_ar,
                display_latin: p.display_latin,
            })
            .collect())
    })
}

#[tauri::command]
fn party_search(
    state: State<'_, AppState>,
    surname: Option<String>,
    given: Option<String>,
) -> Result<Vec<PartyDto>, notaire_core::UiError> {
    with_session(&state, |db, _session| {
        Ok(notaire_core::party::search_by_name(
            db,
            surname.as_deref(),
            given.as_deref(),
            50,
        )?
        .into_iter()
        .map(|p| PartyDto {
            id: p.id,
            kind: p.kind,
            display_ar: p.display_ar,
            display_latin: p.display_latin,
        })
        .collect())
    })
}

#[tauri::command]
fn case_create(
    state: State<'_, AppState>,
    payload: CasePayload,
) -> Result<String, notaire_core::UiError> {
    with_session(&state, |db, session| {
        let principal = session.principal();
        let due_at_ms = payload
            .due_in_days
            .map(|days| notaire_core::now_ms() + days * 86_400_000);
        notaire_core::casefile::create_case(
            db,
            &principal,
            &payload.act_type,
            &payload.owner_user_id,
            payload.confidential,
            due_at_ms,
            None,
        )
    })
}

#[tauri::command]
fn case_list(state: State<'_, AppState>) -> Result<Vec<CaseDto>, notaire_core::UiError> {
    with_session(&state, |db, session| {
        let principal = session.principal();
        Ok(notaire_core::casefile::list_cases(db, &principal, 200)?
            .into_iter()
            .map(|c| CaseDto {
                id: c.id,
                reference: c.reference,
                act_type: c.act_type,
                stage: c.stage,
                opened_at_ms: c.opened_at_ms,
                due_at_ms: c.due_at_ms,
                confidential: c.confidential,
            })
            .collect())
    })
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            office_status,
            office_initialize,
            demo_sign_in,
            auth_login,
            auth_reset_passphrase,
            account_rename,
            account_change_password,
            auth_logout,
            audit_head,
            backup_create,
            backup_verify,
            party_create,
            party_list,
            party_search,
            case_create,
            case_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running the NotaireDZ desktop shell");
}

