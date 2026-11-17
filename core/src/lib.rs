//! `notaire-core` — the domain core of NotaireDZ.
//!
//! Everything security-relevant lives here rather than in the UI: encryption, authorisation, the
//! audit chain, the rules engine and storage. The React front-end only reaches this crate through
//! typed commands, so a permission check cannot be bypassed by driving the interface.

pub mod audit;
pub mod auth;
pub mod backup;
pub mod casefile;
pub mod crypto;
pub mod date;
pub mod error;
pub mod money;
pub mod ocr;
pub mod party;
pub mod rbac;
pub mod recovery;
pub mod registers;
pub mod rules;
pub mod storage;
pub mod vault;

pub use error::{CoreError, Result, UiError};

/// Application version, surfaced in diagnostics and the audit header.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Current time in Unix milliseconds (UTC). One function so tests can reason about time.
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Paths inside the office data directory. Kept in one place so nothing writes elsewhere.
pub struct OfficePaths {
    pub root: std::path::PathBuf,
}

impl OfficePaths {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        OfficePaths { root: root.into() }
    }

    pub fn database(&self) -> std::path::PathBuf {
        self.root.join("office.sqlite")
    }

    pub fn salt(&self) -> std::path::PathBuf {
        self.root.join("office.salt")
    }

    pub fn backups(&self) -> std::path::PathBuf {
        self.root.join("backups")
    }

    pub fn vault(&self) -> std::path::PathBuf {
        self.root.join("vault")
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.backups())?;
        std::fs::create_dir_all(self.vault())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_present() {
        assert!(!super::APP_VERSION.is_empty());
    }

    #[test]
    fn now_is_after_2024() {
        assert!(super::now_ms() > 1_704_067_200_000);
    }
}
