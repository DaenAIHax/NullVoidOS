/// Abstract syntax tree for the `.null` language.
use serde::{Deserialize, Serialize};

/// A source span recorded as a byte offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub offset: usize,
}

/// Top-level expression node — everything in `.null` is an expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Expr {
    // Literals
    Str {
        value: String,
        span: Span,
    },
    Int {
        value: i64,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Null {
        span: Span,
    },

    // Composite
    List {
        items: Vec<Expr>,
        span: Span,
    },
    AttrSet {
        attrs: Vec<Attr>,
        span: Span,
    },

    // References
    /// Bare identifier — in Phase 1 only `pkgs` is valid.
    Ident {
        name: String,
        span: Span,
    },
    /// Field access: `lhs.field`
    FieldAccess {
        lhs: Box<Expr>,
        field: String,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::Str { span, .. } => span,
            Expr::Int { span, .. } => span,
            Expr::Bool { span, .. } => span,
            Expr::Null { span } => span,
            Expr::List { span, .. } => span,
            Expr::AttrSet { span, .. } => span,
            Expr::Ident { span, .. } => span,
            Expr::FieldAccess { span, .. } => span,
        }
    }
}

/// A single `key = value;` binding in an attribute set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attr {
    pub key: String,
    pub key_span: Span,
    pub value: Expr,
}
