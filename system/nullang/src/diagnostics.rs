//! Diagnostics for the Nullang toolchain — NDJSON on stderr, stable codes,
//! typed repair IDs. Inherits `.null`'s contract (SPEC §9), the load-bearing
//! piece of the agent-native thesis: agents apply repairs by `id + args`,
//! never by parsing prose.
use serde::Serialize;

/// Stable error codes. Namespaces per SPEC §9.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiagCode {
    /// Lexical / parse.
    #[serde(rename = "PAR001")]
    Par001,
    /// Unexpected token in parse.
    #[serde(rename = "PAR010")]
    Par010,
    /// Reference resolution — unknown function/identifier.
    #[serde(rename = "REF001")]
    Ref001,
    /// Type mismatch.
    #[serde(rename = "TYP001")]
    Typ001,
    /// Wrong number of call arguments.
    #[serde(rename = "TYP002")]
    Typ002,
    /// Unknown type name.
    #[serde(rename = "TYP003")]
    Typ003,
    /// Non-exhaustive / ill-formed `match`.
    #[serde(rename = "TYP020")]
    Typ020,
    /// Enum variant payload arity mismatch (construction or `match` arm).
    #[serde(rename = "TYP021")]
    Typ021,
    /// Unknown enum symbol.
    #[serde(rename = "REF010")]
    Ref010,
    /// Enum declaration problem (e.g. a symbol used in two enums).
    #[serde(rename = "SCH010")]
    Sch010,
    /// Effect discipline — effectful call without holding the capability.
    #[serde(rename = "EFF001")]
    Eff001,
    /// Missing/ill-formed `main`.
    #[serde(rename = "SCH001")]
    Sch001,
    /// Codegen / cc failure.
    #[serde(rename = "CGN001")]
    Cgn001,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpanInfo {
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// A typed repair: applied by `id` + `args`, not string manipulation
/// (SPEC §9.3).
#[derive(Debug, Clone, Serialize)]
pub struct Repair {
    pub id: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diag {
    pub code: DiagCode,
    pub level: DiagLevel,
    pub message: String,
    pub expected: String,
    pub actual: String,
    pub file: String,
    pub span: SpanInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair: Option<Repair>,
}

impl Diag {
    /// Construct an error diagnostic at a 1-based (line, col).
    pub fn error(
        code: DiagCode,
        message: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        file: &str,
        line: usize,
        col: usize,
    ) -> Self {
        Diag {
            code,
            level: DiagLevel::Error,
            message: message.into(),
            expected: expected.into(),
            actual: actual.into(),
            file: file.to_string(),
            span: SpanInfo {
                line,
                col,
                end_line: line,
                end_col: col,
            },
            repair: None,
        }
    }

    pub fn with_repair(mut self, id: impl Into<String>, args: serde_json::Value) -> Self {
        self.repair = Some(Repair {
            id: id.into(),
            args,
        });
        self
    }
}

/// Emit a diagnostic as one NDJSON line on stderr (SPEC §9.1).
pub fn emit(diag: &Diag) {
    eprintln!(
        "{}",
        serde_json::to_string(diag).expect("diagnostic serialization never fails")
    );
}
