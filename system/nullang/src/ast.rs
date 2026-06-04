//! Abstract syntax tree for Nullang's construction core (v0.1).
//!
//! Deliberately small: top-level `fn` items, a block of statements, and a
//! minimal expression set (literals, identifiers, calls). Arithmetic,
//! `if`/`match`, structs and enums are SPEC §11 deferrals — the v0.1
//! milestone (§13) only needs enough to compile `hello.null`.
use serde::Serialize;

/// Byte-offset span into the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub offset: usize,
}

/// The element type a `List` may carry (SPEC §11 collections, v0.3/v0.4). Kept
/// a small `Copy` enum on purpose: it lets `Ty` stay `Copy` (no heap `Box` in
/// the type representation), so the existing checker/codegen that copy `Ty`
/// freely need no rework. Elements are the scalars (Int/Bool/String) plus a
/// struct **handle** (v0.4) — a struct is a pointer, which fits the same
/// uniform 64-bit slot a String pointer does, so `List<struct>` is nearly free.
/// Nested lists and lists of enums remain deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ElemTy {
    Int,
    Bool,
    String,
    /// A struct element, stored as its `nlstruct<id>` handle (SPEC §11, v0.4).
    Struct(u32),
}

impl ElemTy {
    /// The `Ty` an element decays to when read out of a list.
    pub fn as_ty(self) -> Ty {
        match self {
            ElemTy::Int => Ty::Int,
            ElemTy::Bool => Ty::Bool,
            ElemTy::String => Ty::String,
            ElemTy::Struct(id) => Ty::Struct(id),
        }
    }

    /// The element `Ty` written between `List<` and `>`. Scalars and structs
    /// are allowed; everything else (List, enum, World, Unit) is not.
    pub fn from_ty(t: Ty) -> Option<ElemTy> {
        match t {
            Ty::Int => Some(ElemTy::Int),
            Ty::Bool => Some(ElemTy::Bool),
            Ty::String => Some(ElemTy::String),
            Ty::Struct(id) => Some(ElemTy::Struct(id)),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        self.as_ty().name()
    }
}

/// The semantic types Nullang v0.1 understands. `World` is the capability
/// token (SPEC §5); it is erased at codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Ty {
    Int,
    Bool,
    String,
    Unit,
    World,
    /// A user-declared enum, identified by its index in the enum table.
    /// Lowers to `long` in C (SPEC §7); identity matters only to the checker.
    Enum(u32),
    /// A growable, mutable list of `ElemTy` (SPEC §11). Lowers to the `nl_list`
    /// handle in C — a heap header with reference semantics, so `push`/`set`
    /// mutate in place.
    List(ElemTy),
    /// A user-declared struct, identified by its index in the struct table
    /// (SPEC §11, v0.4). **Reference semantics**: a value is a heap handle
    /// (`nlstruct<id>` — a pointer in C), so field writes mutate through it and
    /// a struct fits the uniform 64-bit list slot. The per-id C name is filled
    /// in by codegen's `c_type_of`; the static `c_type()` returns a placeholder.
    Struct(u32),
}

impl Ty {
    /// The C type this lowers to (SPEC §7). `World` never reaches codegen.
    pub fn c_type(self) -> &'static str {
        match self {
            Ty::Int => "long",
            Ty::Bool => "int",
            Ty::String => "const char*",
            Ty::Unit => "void",
            Ty::World => "void", // erased; should not appear
            Ty::Enum(_) => "long",
            Ty::List(_) => "nl_list",
            Ty::Struct(_) => "void*", // placeholder; codegen overrides per id
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Ty::Int => "Int",
            Ty::Bool => "Bool",
            Ty::String => "String",
            Ty::Unit => "Unit",
            Ty::World => "World",
            Ty::Enum(_) => "enum",
            Ty::List(_) => "List",
            Ty::Struct(_) => "struct",
        }
    }
}

/// One variant of an enum: a symbol, optionally carrying a single typed
/// payload (SPEC §4.2). `.always` has no payload; `.ok(Int)` carries one.
#[derive(Debug, Clone, Serialize)]
pub struct Variant {
    pub name: String,
    /// The payload type as written, or `None` for a bare variant. The
    /// checker restricts the resolved type to Int/Bool/String (SPEC §4.2).
    pub payload: Option<TypeRef>,
    pub span: Span,
}

/// A user-declared closed symbol set (SPEC §4.2):
/// `enum Result = .ok(Int) | .err(String) | .pending`.
#[derive(Debug, Clone, Serialize)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<Variant>,
    pub span: Span,
}

/// One arm of a `match`: `.symbol => expr`, or `.symbol(binder) => expr`
/// when the variant carries a payload (SPEC §4.5). `binder` is `_` to
/// discard the payload.
#[derive(Debug, Clone, Serialize)]
pub struct MatchArm {
    pub symbol: String,
    /// Name binding the matched payload, or `None` for a payload-free arm.
    pub binder: Option<String>,
    pub body: Box<Expr>,
    pub span: Span,
}

/// A type as written in source: a resolved `Ty` when recognised, plus the
/// raw name and span so the checker can report unknown types precisely.
///
/// `elem` carries the element type of a `List<T>` when `T` is not resolvable by
/// the parser alone (a struct name needs the checker's struct table). For a
/// scalar `List<Int>` the parser already fills `resolved`; for `List<Point>` it
/// leaves `resolved` empty and stashes the element `TypeRef` here so the checker
/// can finish resolution. `None` for every non-List type.
#[derive(Debug, Clone, Serialize)]
pub struct TypeRef {
    pub resolved: Option<Ty>,
    pub name: String,
    pub span: Span,
    pub elem: Option<Box<TypeRef>>,
}

/// A capability value, sharing `.null`'s vocabulary (SPEC §5.5):
/// `!net`, `!fs.read."/etc"`, `!tty`.
#[derive(Debug, Clone, Serialize)]
pub struct Capability {
    pub path: Vec<String>,
    pub arg: Option<String>,
    pub span: Span,
}

impl Capability {
    /// Canonical key for subset comparison: `fs.read."/etc"`, `tty`, …
    pub fn key(&self) -> String {
        let mut k = self.path.join(".");
        if let Some(a) = &self.arg {
            k.push_str(&format!(".{:?}", a));
        }
        k
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Func {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: TypeRef,
    /// Declared effects (the `uses` clause). Empty means pure.
    pub uses: Vec<Capability>,
    pub body: Block,
    pub span: Span,
}

/// One field of a struct: `x: Int` (SPEC §11, v0.4). The field type is
/// resolved to a `Ty` by the checker (restricted to Int/Bool/String/struct).
#[derive(Debug, Clone, Serialize)]
pub struct FieldDef {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

/// A user-declared nominal record (SPEC §11, v0.4):
/// `type Point = { x: Int, y: Int };`. Reference semantics (see `Ty::Struct`).
#[derive(Debug, Clone, Serialize)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "item")]
pub enum Item {
    Func(Func),
    Enum(EnumDef),
    Struct(StructDef),
}

#[derive(Debug, Clone, Serialize)]
pub struct File {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Trailing expression (the block's value), if present.
    pub tail: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stmt")]
pub enum Stmt {
    Let {
        name: String,
        /// `let mut` — the binding may be reassigned (SPEC §11 mutable state).
        /// Plain `let` bindings reject assignment (TYP).
        mutable: bool,
        ty: Option<TypeRef>,
        value: Expr,
        span: Span,
    },
    /// `name = expr;` — reassign a `let mut` binding in scope.
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },
    /// `base.field = expr;` — write a struct field through its handle (SPEC
    /// §11, v0.4). `target` is a field-access chain (`p.x`, `p.a.b`); the root
    /// binding must be `let mut`. Index lvalues (`xs[i] = v`) are deliberately
    /// not a form — list writes go through `set` (§10, one way per concept).
    FieldAssign {
        target: Box<Expr>,
        value: Expr,
        span: Span,
    },
    /// `while cond { .. }` — loop while `cond` (a Bool) holds. A statement
    /// (runs for effect); pairs with `let mut` to iterate without recursion.
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
        span: Span,
    },
}

/// Binary operators (SPEC §4: arithmetic, comparison, logical).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOp {
    /// The C operator this lowers to.
    pub fn c_op(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }

    pub fn is_arithmetic(self) -> bool {
        matches!(self, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod)
    }

    pub fn is_ordering(self) -> bool {
        matches!(self, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }

    pub fn is_equality(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne)
    }

    pub fn is_logical(self) -> bool {
        matches!(self, BinOp::And | BinOp::Or)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UnOp {
    Neg,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "expr")]
pub enum Expr {
    Str { value: String, span: Span },
    Int { value: i64, span: Span },
    Bool { value: bool, span: Span },
    Ident { name: String, span: Span },
    Call { callee: String, args: Vec<Expr>, span: Span },
    Unary { op: UnOp, operand: Box<Expr>, span: Span },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    /// `if cond { .. } else { .. }` — an expression; both branches required
    /// (SPEC §4.5).
    If {
        cond: Box<Expr>,
        then_blk: Box<Block>,
        else_blk: Box<Block>,
        span: Span,
    },
    /// An enum value: `.always`, or `.ok(expr)` for a payload variant
    /// (SPEC §4.2, §5.3). `arg` is `None` for a bare variant.
    Symbol { name: String, arg: Option<Box<Expr>>, span: Span },
    /// `match scrutinee { .a => e, .b => e }` — exhaustive over an enum
    /// (SPEC §4.5).
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `[a, b, c]` — a list literal (SPEC §11). All elements share one
    /// `ElemTy`; an empty `[]` takes its type from a surrounding annotation.
    ListLit { elems: Vec<Expr>, span: Span },
    /// `base[index]` — read the element at `index` (a postfix on a `List`).
    /// Out-of-range reads return the element default (total, like `substr`).
    Index { base: Box<Expr>, index: Box<Expr>, span: Span },
    /// `Name { field: expr, ... }` — construct a struct (SPEC §11, v0.4). All
    /// fields are required, each exactly once; order is free (named).
    StructLit { name: String, fields: Vec<FieldInit>, span: Span },
    /// `base.field` — read a struct field (a postfix on a `Struct`).
    Field { base: Box<Expr>, field: String, span: Span },
}

/// One field initializer in a struct literal: `x: 1` (SPEC §11, v0.4).
#[derive(Debug, Clone, Serialize)]
pub struct FieldInit {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Str { span, .. }
            | Expr::Int { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Ident { span, .. }
            | Expr::Call { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::If { span, .. }
            | Expr::Symbol { span, .. }
            | Expr::Match { span, .. }
            | Expr::ListLit { span, .. }
            | Expr::Index { span, .. }
            | Expr::StructLit { span, .. }
            | Expr::Field { span, .. } => *span,
        }
    }
}
