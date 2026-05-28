/// Diagnostic types for the `null` toolchain.
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagCode {
    #[serde(rename = "PAR001")]
    Par001,
    #[serde(rename = "TYP001")]
    Typ001,
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagCode::Par001 => write!(f, "PAR001"),
            DiagCode::Typ001 => write!(f, "TYP001"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagLevel {
    Error,
    Warning,
}

impl fmt::Display for DiagLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagLevel::Error => write!(f, "error"),
            DiagLevel::Warning => write!(f, "warning"),
        }
    }
}

/// A single diagnostic message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diag {
    pub level: DiagLevel,
    pub code: DiagCode,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {} [{}]: {}",
            self.file, self.line, self.col, self.level, self.code, self.message
        )?;
        if let Some(fix) = &self.fix {
            write!(f, "\n  fix: {}", fix)?;
        }
        Ok(())
    }
}

/// Print a diagnostic to stderr.  When `json_mode` is true, emit one
/// JSON object per line; otherwise emit the human-readable format.
pub fn emit(diag: &Diag, json_mode: bool) {
    if json_mode {
        eprintln!("{}", serde_json::to_string(diag).unwrap_or_else(|_| format!("{:?}", diag)));
    } else {
        eprintln!("{}", diag);
    }
}
