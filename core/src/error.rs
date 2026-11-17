//! Error type shared by every core service.
//!
//! Errors carry a translation key, never a raw internal string, so the UI can render
//! the message in Arabic or French without leaking implementation detail.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("storage: {0}")]
    Storage(String),
    #[error("invalid input: {0}")]
    Validation(String),
    #[error("denied")]
    Denied,
    #[error("locked out until {until_ms}")]
    LockedOut { until_ms: i64 },
    #[error("re-authentication required")]
    ReauthRequired,
    #[error("audit chain broken at sequence {seq}")]
    AuditBroken { seq: u64 },
    #[error("io: {0}")]
    Io(String),
    #[error("rules: {0}")]
    Rules(String),
}

/// An error as the UI receives it: a stable code plus a translation key.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UiError {
    pub code: String,
    pub message_key: String,
    pub detail: Option<String>,
}

impl From<CoreError> for UiError {
    fn from(e: CoreError) -> Self {
        let (code, key) = match &e {
            CoreError::Crypto(_) => ("crypto", "error.crypto"),
            CoreError::Storage(_) => ("storage", "error.storage"),
            CoreError::Validation(_) => ("validation", "error.validation"),
            CoreError::Denied => ("denied", "error.denied"),
            CoreError::LockedOut { .. } => ("locked_out", "error.locked_out"),
            CoreError::ReauthRequired => ("reauth", "error.reauth_required"),
            CoreError::AuditBroken { .. } => ("audit_broken", "error.audit_broken"),
            CoreError::Io(_) => ("io", "error.io"),
            CoreError::Rules(_) => ("rules", "error.rules"),
        };
        UiError {
            code: code.to_string(),
            message_key: key.to_string(),
            // Validation messages are written for humans; everything else stays opaque.
            detail: match &e {
                CoreError::Validation(m) => Some(m.clone()),
                _ => None,
            },
        }
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Storage(e.to_string())
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
