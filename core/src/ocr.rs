//! OCR behind an interface.
//!
//! Recognition is never part of the trusted path: it runs in a separate offline process, produces
//! text that a human must confirm, and the engine can be absent. `UnavailableOcr` is a first-class
//! state, not an error to hide — a machine without Tesseract must say so instead of pretending.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub engine: String,
    pub languages: Vec<String>,
    /// Always true: OCR output is a proposal, never a fact.
    pub requires_human_confirmation: bool,
}

pub trait OcrEngine: Send + Sync {
    fn name(&self) -> String;
    fn languages(&self) -> Vec<String>;
    fn is_available(&self) -> bool;
    fn recognize(&self, bytes: &[u8], extension: &str) -> Result<OcrResult>;
}

/// Offline Tesseract CLI adapter. No network, no cloud API, no shell interpolation.
pub struct TesseractCli {
    program: PathBuf,
    languages: Vec<String>,
    timeout: Duration,
}

impl Default for TesseractCli {
    fn default() -> Self {
        TesseractCli {
            program: PathBuf::from("tesseract"),
            languages: vec!["ara".to_string(), "fra".to_string()],
            timeout: Duration::from_secs(60),
        }
    }
}

impl TesseractCli {
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        TesseractCli { program: program.into(), ..Default::default() }
    }
}

impl OcrEngine for TesseractCli {
    fn name(&self) -> String {
        "tesseract-cli".to_string()
    }

    fn languages(&self) -> Vec<String> {
        self.languages.clone()
    }

    fn is_available(&self) -> bool {
        Command::new(&self.program)
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    fn recognize(&self, bytes: &[u8], extension: &str) -> Result<OcrResult> {
        if !self.is_available() {
            return Err(CoreError::Validation(
                "ocr engine is not installed on this machine".into(),
            ));
        }
        let ext = match extension {
            "png" | "jpg" | "jpeg" | "tif" | "tiff" | "pdf" => extension,
            _ => "png",
        };
        let mut name = [0u8; 8];
        getrandom::getrandom(&mut name).map_err(|e| CoreError::Crypto(format!("rng: {e}")))?;
        let dir = std::env::temp_dir().join("notaire-ocr");
        std::fs::create_dir_all(&dir)?;
        let input = dir.join(format!("{}.{ext}", hex::encode(name)));
        std::fs::write(&input, bytes)?;

        let started = Instant::now();
        let output = Command::new(&self.program)
            .arg(&input)
            .arg("stdout")
            .args(["-l", "ara+fra"])
            .output()
            .map_err(|e| CoreError::Io(format!("ocr failed to start: {e}")))?;
        let _ = std::fs::remove_file(&input);

        if started.elapsed() > self.timeout {
            return Err(CoreError::Io("ocr timed out".into()));
        }
        if !output.status.success() {
            return Err(CoreError::Io("ocr engine returned an error".into()));
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(OcrResult {
            text,
            engine: self.name(),
            languages: self.languages(),
            requires_human_confirmation: true,
        })
    }
}

/// Used when no offline engine is installed. It never fabricates text.
pub struct UnavailableOcr {
    pub reason: String,
}

impl OcrEngine for UnavailableOcr {
    fn name(&self) -> String {
        "unavailable".to_string()
    }

    fn languages(&self) -> Vec<String> {
        Vec::new()
    }

    fn is_available(&self) -> bool {
        false
    }

    fn recognize(&self, _bytes: &[u8], _extension: &str) -> Result<OcrResult> {
        Err(CoreError::Validation(format!(
            "ocr unavailable: {}",
            self.reason
        )))
    }
}

/// Run OCR for a stored document and persist the text encrypted, through the vault module.
pub fn run_and_store(
    engine: &dyn OcrEngine,
    read: impl FnOnce() -> Result<Vec<u8>>,
    store: impl FnOnce(&str) -> Result<()>,
    extension: &str,
) -> Result<OcrResult> {
    if !engine.is_available() {
        return Err(CoreError::Validation(format!(
            "ocr engine {} is not available",
            engine.name()
        )));
    }
    let bytes = read()?;
    let result = engine.recognize(&bytes, extension)?;
    store(&result.text)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeEngine {
        text: String,
        stored: Mutex<Option<String>>,
    }

    impl OcrEngine for FakeEngine {
        fn name(&self) -> String {
            "fake".into()
        }
        fn languages(&self) -> Vec<String> {
            vec!["ara".into(), "fra".into()]
        }
        fn is_available(&self) -> bool {
            true
        }
        fn recognize(&self, _bytes: &[u8], _ext: &str) -> Result<OcrResult> {
            Ok(OcrResult {
                text: self.text.clone(),
                engine: self.name(),
                languages: self.languages(),
                requires_human_confirmation: true,
            })
        }
    }

    #[test]
    fn pipeline_stores_recognised_text() {
        let engine = FakeEngine {
            text: "عقد بيع فريض".into(),
            stored: Mutex::new(None),
        };
        let result = run_and_store(
            &engine,
            || Ok(b"image".to_vec()),
            |text| {
                *engine.stored.lock().unwrap() = Some(text.to_string());
                Ok(())
            },
            "png",
        )
        .unwrap();
        assert_eq!(result.text, "عقد بيع فريض");
        assert!(result.requires_human_confirmation);
        assert_eq!(engine.stored.lock().unwrap().as_deref(), Some("عقد بيع فريض"));
    }

    #[test]
    fn unavailable_engine_reports_plainly_and_never_fabricates() {
        let engine = UnavailableOcr { reason: "tesseract not installed".into() };
        assert!(!engine.is_available());
        let error = engine.recognize(b"x", "png").unwrap_err();
        assert!(matches!(error, CoreError::Validation(_)));
        assert!(engine.languages().is_empty());
    }

    #[test]
    fn missing_engine_is_refused_before_any_read() {
        let engine = UnavailableOcr { reason: "absent".into() };
        let read_called = Mutex::new(false);
        let result = run_and_store(
            &engine,
            || {
                *read_called.lock().unwrap() = true;
                Ok(b"never".to_vec())
            },
            |_| Ok(()),
            "png",
        );
        assert!(result.is_err());
        assert!(!*read_called.lock().unwrap(), "no bytes should be read without an engine");
    }
}
