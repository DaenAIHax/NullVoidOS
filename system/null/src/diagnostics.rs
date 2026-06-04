/// Diagnostic types for the `null` toolchain (v2).
///
/// Shape per SPEC §7.1: one JSON object per diagnostic, NDJSON on stderr.
/// Stable error codes (SPEC §7.2) live in `DiagCode`; typed repair IDs
/// (SPEC §7.3) live in `Repair`.
///
/// The load-bearing piece is `Repair`: agents apply repairs by `id + args`
/// — not by parsing prose.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error codes. Namespaces per SPEC §7.2:
///   PAR — lexical / parse
///   TYP — type mismatch
///   SCH — schema (missing/unknown fields)
///   REF — reference resolution (`pkgs.X`)
///   CAP — capability (unknown cap, system whitelist violation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagCode {
    #[serde(rename = "PAR001")]
    Par001,
    #[serde(rename = "TYP001")]
    Typ001,
    #[serde(rename = "TYP004")]
    Typ004,
    #[serde(rename = "SCH001")]
    Sch001,
    #[serde(rename = "REF002")]
    Ref002,
    #[serde(rename = "CAP001")]
    Cap001,
    #[serde(rename = "CAP004")]
    Cap004,
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagCode::Par001 => write!(f, "PAR001"),
            DiagCode::Typ001 => write!(f, "TYP001"),
            DiagCode::Typ004 => write!(f, "TYP004"),
            DiagCode::Sch001 => write!(f, "SCH001"),
            DiagCode::Ref002 => write!(f, "REF002"),
            DiagCode::Cap001 => write!(f, "CAP001"),
            DiagCode::Cap004 => write!(f, "CAP004"),
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

/// Structured source position. End offsets are best-effort: when a node
/// only tracks a start offset, `end_*` collapse onto `*`. Future spans
/// (parser-level rather than node-level) will fill them in exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanInfo {
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// Typed repair instruction. SPEC §7.3. The agent applies a repair by
/// `id + args` — both are stable, versioned, and machine-actionable.
///
/// `id` is a stable kebab-case identifier. It is owned (`String`) rather
/// than `&'static str` so the type round-trips through Deserialize, but
/// the construction-time API (`Repair::new`) takes `&'static str` to
/// keep the call sites unambiguous about which set of IDs they belong to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repair {
    pub id: String,
    pub args: serde_json::Value,
}

impl Repair {
    pub fn new(id: &'static str, args: serde_json::Value) -> Self {
        Repair {
            id: id.to_string(),
            args,
        }
    }
}

/// A single diagnostic message. NDJSON-serialized on stderr.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diag {
    pub level: DiagLevel,
    pub code: DiagCode,
    pub message: String,
    pub expected: String,
    pub actual: String,
    pub file: String,
    pub span: SpanInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair: Option<Repair>,
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {} [{}]: {}\n  expected: {}\n  actual:   {}",
            self.file,
            self.span.line,
            self.span.col,
            self.level,
            self.code,
            self.message,
            self.expected,
            self.actual,
        )?;
        if let Some(repair) = &self.repair {
            write!(f, "\n  repair:   {} {}", repair.id, repair.args)?;
        }
        Ok(())
    }
}

/// Emit a diagnostic to stderr. v2 default is NDJSON. The `json` parameter
/// is retained as a kill-switch for the human-readable fallback used by
/// integration tests that snapshot prose.
pub fn emit(diag: &Diag, json: bool) {
    if json {
        eprintln!(
            "{}",
            serde_json::to_string(diag).unwrap_or_else(|_| format!("{:?}", diag))
        );
    } else {
        eprintln!("{}", diag);
    }
}

// ---------------------------------------------------------------------------
// Constructor helpers — keep call sites concise
// ---------------------------------------------------------------------------

/// Build a SpanInfo from a single point (line, col). End collapses to start.
pub fn span_at(line: usize, col: usize) -> SpanInfo {
    SpanInfo {
        line,
        col,
        end_line: line,
        end_col: col,
    }
}
