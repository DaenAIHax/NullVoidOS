//! Semantic checks for Nullang. The load-bearing one is **effect discipline**
//! (SPEC §5): a function may only call effectful functions whose effects its
//! own `uses` clause covers. Also: `main`'s shape (§4.6), call resolution,
//! argument and operator types, `if`/`match` typing, and enum exhaustiveness.
//!
//! v0.1 restriction: enum symbols are **globally unique** — a `.symbol`
//! names exactly one enum. This keeps typing bottom-up (no expected-type
//! threading) and matches the "one way to express each concept" rule.
//!
//! Returns a `Checked` (signatures + symbol→index map) for codegen.
use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::{Diag, DiagCode};
use crate::lexer::line_col;

/// A resolved function signature.
#[derive(Debug, Clone)]
pub struct Sig {
    pub params: Vec<Ty>,
    pub ret: Ty,
    /// Capability keys this function may exercise (SPEC §5).
    pub effects: Vec<String>,
    /// Name to emit in C (builtins rename, e.g. `print` → `nullang_print`).
    pub c_name: String,
}

pub type SigTable = HashMap<String, Sig>;

/// One enum variant, resolved: a symbol with an optional payload type
/// (SPEC §4.2). Payload types are restricted to Int/Bool/String.
#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: String,
    pub payload: Option<Ty>,
}

/// A resolved enum. `tagged` is true when any variant carries a payload —
/// codegen then lowers it to a `{ tag, union }` struct rather than a bare
/// `long` (SPEC §7).
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<VariantInfo>,
    pub tagged: bool,
}

/// A resolved struct: its fields in declaration order with resolved types
/// (SPEC §11, v0.4). Reference semantics — lowers to a heap-allocated
/// `nlstruct<id>` handle in C (codegen).
#[derive(Debug, Clone)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<(String, Ty)>,
}

impl StructInfo {
    fn field(&self, name: &str) -> Option<(usize, Ty)> {
        self.fields
            .iter()
            .enumerate()
            .find(|(_, (n, _))| n == name)
            .map(|(i, (_, t))| (i, *t))
    }
}

/// Output of checking: signatures, the enum-symbol → (enum id, variant
/// index) map, and the resolved enum + struct tables. Codegen lowers a
/// payload-free symbol to its index; a payload-carrying enum lowers to a
/// tagged union; a struct lowers to a heap handle.
pub struct Checked {
    pub sigs: SigTable,
    pub symbols: HashMap<String, (u32, usize)>,
    pub enums: Vec<EnumInfo>,
    pub structs: Vec<StructInfo>,
}

/// Seed the builtins available without declaration (SPEC §5, §13).
fn builtins() -> SigTable {
    let mut t = SigTable::new();
    t.insert(
        "print".to_string(),
        Sig {
            params: vec![Ty::World, Ty::String],
            ret: Ty::Unit,
            effects: vec!["tty".to_string()],
            c_name: "nullang_print".to_string(),
        },
    );
    // Explicit string composition (SPEC §10: "compose explicitly"). Pure —
    // no `World`, no effects. `concat` is strictly BINARY (no variadics /
    // operator overloading, §10); deep nesting is the intended cost.
    t.insert(
        "concat".to_string(),
        Sig {
            params: vec![Ty::String, Ty::String],
            ret: Ty::String,
            effects: vec![],
            c_name: "nullang_concat".to_string(),
        },
    );
    t.insert(
        "str_of_int".to_string(),
        Sig {
            params: vec![Ty::Int],
            ret: Ty::String,
            effects: vec![],
            c_name: "nullang_str_of_int".to_string(),
        },
    );
    // Tier 0 — string DEcomposition (the dual of `concat`). Pure. `concat`
    // builds up; these take apart, the wall the editor experiment hit first.
    t.insert(
        "str_len".to_string(),
        Sig {
            params: vec![Ty::String],
            ret: Ty::Int,
            effects: vec![],
            c_name: "nullang_str_len".to_string(),
        },
    );
    // `substr(s, start, len) -> String`. Indices clamp to bounds, so it is
    // total (no panics, no error type needed). `char_at(s,i)` is `substr(s,i,1)`.
    t.insert(
        "substr".to_string(),
        Sig {
            params: vec![Ty::String, Ty::Int, Ty::Int],
            ret: Ty::String,
            effects: vec![],
            c_name: "nullang_substr".to_string(),
        },
    );
    // char_at(s, i) — the 1-char String at index i. Equivalent to
    // substr(s, i, 1) but O(i) not O(n): it stops at i instead of strlen-ing
    // the whole string, so a left-to-right scan is O(n) not O(n^2). "" out of
    // range. AUTHORED BY THE IN-VM AGENT (first self-served builtin, within
    // BUILTINS_CONTRACT.md); merged + verified host-side.
    t.insert(
        "char_at".to_string(),
        Sig {
            params: vec![Ty::String, Ty::Int],
            ret: Ty::String,
            effects: vec![],
            c_name: "nullang_char_at".to_string(),
        },
    );
    // P0 stdlib (the String<->Int seam two probes — lexer + config-parser —
    // both hit). Pure, total. `char_code(s,i)` is the missing `char->Int`:
    // the byte value at index i, so character classes become arithmetic
    // ranges (`c >= 48 && c <= 57`) instead of 10-arm `==` chains. `-1` out of
    // range (a real byte is 0..255, so -1 is an unambiguous sentinel, unlike
    // char_at's "" which collides with a NUL).
    t.insert(
        "char_code".to_string(),
        Sig {
            params: vec![Ty::String, Ty::Int],
            ret: Ty::Int,
            effects: vec![],
            c_name: "nullang_char_code".to_string(),
        },
    );
    // `int_of_str(s) -> Int`: decimal parse, total (no Result yet, like
    // read_file). Confirmed the headline deterministic gap 3/3 across probes —
    // every config-parser hand-rolled the same ~22-LOC parse_int. Leading
    // optional `-`, then digits; stops at the first non-digit; "" or junk -> 0.
    // A Result-returning variant (to tell "0" from a parse error) is the §10
    // follow-up, same as read_file.
    t.insert(
        "int_of_str".to_string(),
        Sig {
            params: vec![Ty::String],
            ret: Ty::Int,
            effects: vec![],
            c_name: "nullang_int_of_str".to_string(),
        },
    );
    // P1 stdlib — search + split (both probes reached for these). Authored by
    // the in-VM agent (BUILTINS_CONTRACT, generation-7); merged host verbatim.
    // `index_of(s, sub) -> Int`: byte index of the first occurrence of `sub`
    // in `s`, or -1 if absent. Empty `sub` returns 0 (matches at the start,
    // the standard convention — the dual of how splitting on "" behaves).
    // Total: no panics. The "find" half of search/replace; pairs with `substr`.
    t.insert(
        "index_of".to_string(),
        Sig {
            params: vec![Ty::String, Ty::String],
            ret: Ty::Int,
            effects: vec![],
            c_name: "nullang_index_of".to_string(),
        },
    );
    // `split(s, sep) -> List<String>`: splits `s` on every occurrence of `sep`.
    // First builtin to PRODUCE a List<T> — uses the existing nl_list runtime
    // (push String pointers via intptr_t, same boxing as user code). Edge cases:
    // empty `sep` yields `[s]` (one-element list — splitting on nothing leaves
    // the input intact, sidesteps the "infinite empties" trap); consecutive
    // separators yield empty segments (so `split("a,,b", ",")` is [a, "", b]).
    t.insert(
        "split".to_string(),
        Sig {
            params: vec![Ty::String, Ty::String],
            ret: Ty::List(ElemTy::String),
            effects: vec![],
            c_name: "nullang_split".to_string(),
        },
    );
    // `join(parts, sep) -> String`: dual of `split`. Concatenates every element
    // of `parts` (a `List<String>`), inserting `sep` between consecutive
    // elements. Total: empty list -> ""; single-element list -> that element
    // (sep is never inserted); empty sep -> straight concatenation. Pairs with
    // `split` so `join(split(s, sep), sep) == s` whenever `sep` is non-empty.
    // AUTHORED BY THE IN-VM AGENT under BUILTINS_CONTRACT.md (Sig only); folded
    // host-side (Via B) — the agent cannot push.
    t.insert(
        "join".to_string(),
        Sig {
            params: vec![Ty::List(ElemTy::String), Ty::String],
            ret: Ty::String,
            effects: vec![],
            c_name: "nullang_join".to_string(),
        },
    );
    // `replace(s, from, to) -> String`: substitutes every leftmost,
    // non-overlapping occurrence of `from` in `s` with `to`. Total: `from == ""`
    // returns a fresh copy of `s` (no splitting on nothing — same convention as
    // `split`); `from` absent returns `s` unchanged; `to == ""` deletes. After
    // each match the scan resumes AFTER the replaced segment, so
    // `replace("aaa", "aa", "b")` is `"ba"` not `"bb"` (the replacement is not
    // re-scanned). Pure. AUTHORED BY THE IN-VM AGENT under BUILTINS_CONTRACT.md;
    // folded host-side (Via B) — the agent cannot push.
    t.insert(
        "replace".to_string(),
        Sig {
            params: vec![Ty::String, Ty::String, Ty::String],
            ret: Ty::String,
            effects: vec![],
            c_name: "nullang_replace".to_string(),
        },
    );
    // Tier 0 — file I/O. Effectful (World-gated, like `print`). The LANGUAGE
    // effect is path-less (`fs.read`/`fs.write`): the path is a runtime
    // String, and the system-level grant in `system.null` scopes it — which
    // is exactly what Landlock then enforces at `nv-rebuild run`. So an fn
    // that reads files declares `uses !fs.read` (no path); the path is data.
    t.insert(
        "read_file".to_string(),
        Sig {
            params: vec![Ty::World, Ty::String],
            ret: Ty::String,
            effects: vec!["fs.read".to_string()],
            c_name: "nullang_read_file".to_string(),
        },
    );
    t.insert(
        "write_file".to_string(),
        Sig {
            params: vec![Ty::World, Ty::String, Ty::String],
            ret: Ty::Unit,
            effects: vec!["fs.write".to_string()],
            c_name: "nullang_write_file".to_string(),
        },
    );
    // The first *non-deterministic* builtin and the first to exercise `!time`
    // (SPEC §5 vocabulary). Effectful, World-gated like file I/O. Returns Unix
    // time in **whole seconds** as an Int (the VM is LP64, so `long` is 64-bit:
    // no Y2038). The `!time` effect is static-only by necessity — `time()` is a
    // vDSO read, not a syscall seccomp can usefully gate — so it audits intent,
    // it does not sandbox. MUST NOT appear in any smoke-probe equality check:
    // its value differs every run by design (see selfhost-bootstrap discipline).
    t.insert(
        "now".to_string(),
        Sig {
            params: vec![Ty::World],
            ret: Ty::Int,
            effects: vec!["time".to_string()],
            c_name: "nullang_now".to_string(),
        },
    );
    // Process arguments (Wave 2 — gate for `cat <file>`/`grep`/`sed`-likes).
    // Pure: argv is startup data the runtime provides, like a constant — no
    // World, no effect (and so no `!proc.argv` to add to `null`'s vocabulary).
    // C convention: `argv(0)` is the program name; `argc()` counts it;
    // out-of-range `argv(i)` returns "". `List<String>` is the §11 ergonomic
    // form, deferred with the rest of the collection work.
    t.insert(
        "argc".to_string(),
        Sig {
            params: vec![],
            ret: Ty::Int,
            effects: vec![],
            c_name: "nullang_argc".to_string(),
        },
    );
    t.insert(
        "argv".to_string(),
        Sig {
            params: vec![Ty::Int],
            ret: Ty::String,
            effects: vec![],
            c_name: "nullang_argv".to_string(),
        },
    );
    t
}

pub fn check_file(file: &File, src: &str, fname: &str) -> Result<Checked, Diag> {
    // Pass 0: collect enums (with resolved, restricted payload types) and
    // the globally-unique symbol map.
    let mut enums: Vec<EnumInfo> = Vec::new();
    let mut enum_by_name: HashMap<String, u32> = HashMap::new();
    let mut symbols: HashMap<String, (u32, usize)> = HashMap::new();
    for item in &file.items {
        if let Item::Enum(e) = item {
            let (line, col) = line_col(src, e.span.offset);
            if enum_by_name.contains_key(&e.name) {
                return Err(Diag::error(
                    DiagCode::Sch010,
                    format!("enum `{}` declared more than once", e.name),
                    "a unique enum name",
                    e.name.clone(),
                    fname,
                    line,
                    col,
                ));
            }
            let id = enums.len() as u32;
            let mut variants = Vec::new();
            let mut tagged = false;
            for (idx, v) in e.variants.iter().enumerate() {
                if let Some((other, _)) = symbols.get(&v.name) {
                    return Err(Diag::error(
                        DiagCode::Sch010,
                        format!(
                            "symbol `.{}` is declared in two enums (`{}` and `{}`); symbols must be globally unique in v0.1",
                            v.name,
                            enums[*other as usize].name,
                            e.name
                        ),
                        "a globally-unique symbol",
                        format!(".{}", v.name),
                        fname,
                        line,
                        col,
                    ));
                }
                let payload = match &v.payload {
                    None => None,
                    Some(tref) => Some(resolve_payload_ty(tref, src, fname)?),
                };
                if payload.is_some() {
                    tagged = true;
                }
                symbols.insert(v.name.clone(), (id, idx));
                variants.push(VariantInfo {
                    name: v.name.clone(),
                    payload,
                });
            }
            enum_by_name.insert(e.name.clone(), id);
            enums.push(EnumInfo {
                name: e.name.clone(),
                variants,
                tagged,
            });
        }
    }

    // Pass 0b: register all struct names first (so fields may reference any
    // struct — forward and self-reference through the handle), then resolve
    // each struct's fields. Struct and enum names share one type namespace.
    let mut struct_by_name: HashMap<String, u32> = HashMap::new();
    for item in &file.items {
        if let Item::Struct(s) = item {
            let (line, col) = line_col(src, s.span.offset);
            if struct_by_name.contains_key(&s.name) || enum_by_name.contains_key(&s.name) {
                return Err(Diag::error(
                    DiagCode::Sch010,
                    format!("type `{}` declared more than once", s.name),
                    "a unique type name",
                    s.name.clone(),
                    fname,
                    line,
                    col,
                ));
            }
            struct_by_name.insert(s.name.clone(), struct_by_name.len() as u32);
        }
    }
    let mut structs: Vec<StructInfo> = Vec::new();
    for item in &file.items {
        if let Item::Struct(s) = item {
            let mut fields = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            for f in &s.fields {
                if !seen.insert(f.name.clone()) {
                    let (line, col) = line_col(src, f.span.offset);
                    return Err(Diag::error(
                        DiagCode::Sch010,
                        format!("field `{}` declared twice in struct `{}`", f.name, s.name),
                        "a unique field name",
                        f.name.clone(),
                        fname,
                        line,
                        col,
                    ));
                }
                let ty = resolve_field_ty(&f.ty, &enum_by_name, &struct_by_name, src, fname)?;
                fields.push((f.name.clone(), ty));
            }
            structs.push(StructInfo {
                name: s.name.clone(),
                fields,
            });
        }
    }

    // Pass 1: collect user function signatures.
    let mut sigs = builtins();
    for item in &file.items {
        if let Item::Func(f) = item {
            let mut params = Vec::new();
            for p in &f.params {
                params.push(resolve_ty(&p.ty, &enum_by_name, &struct_by_name, src, fname)?);
            }
            let ret = resolve_ty(&f.ret, &enum_by_name, &struct_by_name, src, fname)?;
            let effects = f.uses.iter().map(|c| c.key()).collect();
            sigs.insert(
                f.name.clone(),
                Sig {
                    params,
                    ret,
                    effects,
                    // Mangle user fn names so a C keyword (`double`, `int`, …)
                    // or a runtime-symbol clash can't reach the emitted C.
                    // Builtins keep their `nullang_*` c_name; `main` is erased
                    // by mangle. Call sites read this, so defs and calls agree.
                    c_name: crate::codegen::mangle(&f.name),
                },
            );
        }
    }

    // Pass 2: validate each function body.
    let checker = Checker {
        src,
        fname,
        sigs: &sigs,
        enums: &enums,
        symbols: &symbols,
        structs: &structs,
    };
    for item in &file.items {
        if let Item::Func(f) = item {
            checker.check_func(f)?;
        }
    }

    check_main(file, src, fname)?;
    Ok(Checked {
        sigs,
        symbols,
        enums,
        structs,
    })
}

/// Resolve an enum payload type, restricting it to the v0.2 set
/// (Int/Bool/String). Enum-typed and `World`/`Unit` payloads are rejected
/// — they would require indirection or carry no data (SPEC §4.2, §11).
fn resolve_payload_ty(t: &TypeRef, src: &str, fname: &str) -> Result<Ty, Diag> {
    match t.resolved {
        Some(ty @ (Ty::Int | Ty::Bool | Ty::String)) => Ok(ty),
        _ => {
            let (line, col) = line_col(src, t.span.offset);
            Err(Diag::error(
                DiagCode::Sch010,
                format!(
                    "enum payload type `{}` is not allowed; v0.2 payloads are Int, Bool, or String",
                    t.name
                ),
                "Int, Bool, or String",
                t.name.clone(),
                fname,
                line,
                col,
            ))
        }
    }
}

fn resolve_ty(
    t: &TypeRef,
    enum_by_name: &HashMap<String, u32>,
    struct_by_name: &HashMap<String, u32>,
    src: &str,
    fname: &str,
) -> Result<Ty, Diag> {
    if let Some(ty) = t.resolved {
        return Ok(ty);
    }
    // `List<struct>`: the parser couldn't resolve the element (no struct table),
    // so it stashed the element `TypeRef` in `elem`. Resolve it now; only a
    // scalar or a struct is a legal element (List/enum elements are deferred).
    if let Some(elem) = &t.elem {
        let elem_ty = resolve_ty(elem, enum_by_name, struct_by_name, src, fname)?;
        match ElemTy::from_ty(elem_ty) {
            Some(et) => return Ok(Ty::List(et)),
            None => {
                let (line, col) = line_col(src, elem.span.offset);
                return Err(Diag::error(
                    DiagCode::Typ003,
                    format!(
                        "`{}` is not a valid list element type; elements are Int, Bool, String, or a struct",
                        elem.name
                    ),
                    "Int, Bool, String, or a struct",
                    elem.name.clone(),
                    fname,
                    line,
                    col,
                ));
            }
        }
    }
    if let Some(id) = enum_by_name.get(&t.name) {
        return Ok(Ty::Enum(*id));
    }
    if let Some(id) = struct_by_name.get(&t.name) {
        return Ok(Ty::Struct(*id));
    }
    let (line, col) = line_col(src, t.span.offset);
    Err(Diag::error(
        DiagCode::Typ003,
        format!("unknown type `{}`", t.name),
        "Int, Bool, String, Unit, World, or a declared enum/struct",
        t.name.clone(),
        fname,
        line,
        col,
    ))
}

/// Resolve a struct field type, restricting it to the v0.4 set: Int, Bool,
/// String, or another struct (by handle). Enum-typed and List-typed fields are
/// deferred — they lower cleanly (both fit the slot) but are held back to keep
/// the first struct cut bounded; `World`/`Unit` are never storable.
fn resolve_field_ty(
    t: &TypeRef,
    enum_by_name: &HashMap<String, u32>,
    struct_by_name: &HashMap<String, u32>,
    src: &str,
    fname: &str,
) -> Result<Ty, Diag> {
    let ty = resolve_ty(t, enum_by_name, struct_by_name, src, fname)?;
    match ty {
        Ty::Int | Ty::Bool | Ty::String | Ty::Struct(_) => Ok(ty),
        _ => {
            let (line, col) = line_col(src, t.span.offset);
            Err(Diag::error(
                DiagCode::Sch010,
                format!(
                    "struct field type `{}` is not allowed; v0.4 fields are Int, Bool, String, or another struct",
                    t.name
                ),
                "Int, Bool, String, or a struct",
                t.name.clone(),
                fname,
                line,
                col,
            ))
        }
    }
}

fn check_main(file: &File, src: &str, fname: &str) -> Result<(), Diag> {
    for item in &file.items {
        if let Item::Func(f) = item {
            if f.name != "main" {
                continue;
            }
            let ok_params = f.params.len() == 1 && f.params[0].ty.resolved == Some(Ty::World);
            let ok_ret = f.ret.resolved == Some(Ty::Int);
            if ok_params && ok_ret {
                return Ok(());
            }
            let (line, col) = line_col(src, f.span.offset);
            return Err(Diag::error(
                DiagCode::Sch001,
                "`main` must take a single `World` parameter and return `Int`",
                "fn main(world: World) -> Int",
                format!("fn main/{} -> {}", f.params.len(), f.ret.name),
                fname,
                line,
                col,
            ));
        }
    }
    let (line, col) = line_col(src, 0);
    Err(Diag::error(
        DiagCode::Sch001,
        "no `main` function found",
        "fn main(world: World) -> Int",
        "no main",
        fname,
        line,
        col,
    ))
}

struct Checker<'a> {
    src: &'a str,
    fname: &'a str,
    sigs: &'a SigTable,
    enums: &'a [EnumInfo],
    symbols: &'a HashMap<String, (u32, usize)>,
    structs: &'a [StructInfo],
}

impl<'a> Checker<'a> {
    fn diag(&self, code: DiagCode, msg: String, expected: &str, actual: String, span: Span) -> Diag {
        let (line, col) = line_col(self.src, span.offset);
        Diag::error(code, msg, expected, actual, self.fname, line, col)
    }

    /// Resolve a written `TypeRef` against the checker's enum/struct tables.
    /// Used where a type annotation must be turned into a `Ty` after pass 1
    /// (e.g. an empty `let xs: List<Point> = []`): a `List<struct>` is left
    /// unresolved by the parser and carries its element in `elem`. Returns
    /// `None` for an unknown or illegal type rather than diagnosing — callers
    /// already report the precise error in context.
    fn resolve_typeref(&self, t: &TypeRef) -> Option<Ty> {
        if let Some(ty) = t.resolved {
            return Some(ty);
        }
        if let Some(elem) = &t.elem {
            // `List<T>` with an unresolved element (a struct): resolve T, then
            // require it to be a legal list element.
            return self.resolve_typeref(elem).and_then(ElemTy::from_ty).map(Ty::List);
        }
        if let Some(i) = self.structs.iter().position(|s| s.name == t.name) {
            return Some(Ty::Struct(i as u32));
        }
        if let Some(i) = self.enums.iter().position(|e| e.name == t.name) {
            return Some(Ty::Enum(i as u32));
        }
        None
    }

    fn check_func(&self, f: &Func) -> Result<(), Diag> {
        let sig = self.sigs.get(&f.name).expect("signature collected in pass 1");
        // Each local maps to (type, mutable). Params are immutable bindings.
        let mut locals: HashMap<String, (Ty, bool)> = HashMap::new();
        for (p, ty) in f.params.iter().zip(sig.params.iter()) {
            locals.insert(p.name.clone(), (*ty, false));
        }
        let uses: HashSet<String> = f.uses.iter().map(|c| c.key()).collect();
        let ret = sig.ret;

        let tail_ty = self.check_block(&f.body, &locals, &uses, &f.name)?;
        if f.body.tail.is_some() && ret != Ty::Unit && tail_ty != ret {
            let span = f.body.tail.as_ref().unwrap().span();
            return Err(self.diag(
                DiagCode::Typ001,
                format!(
                    "`{}` returns {} but its body yields {}",
                    f.name,
                    ret.name(),
                    tail_ty.name()
                ),
                ret.name(),
                tail_ty.name().to_string(),
                span,
            ));
        }
        Ok(())
    }

    /// Check a block in a child scope; returns the type of its trailing
    /// expression (or `Unit` if there is none).
    fn check_block(
        &self,
        block: &Block,
        outer: &HashMap<String, (Ty, bool)>,
        uses: &HashSet<String>,
        caller_name: &str,
    ) -> Result<Ty, Diag> {
        let mut locals = outer.clone();
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let { name, mutable, ty, value, .. } => {
                    // An empty `[]` literal cannot infer its element type from
                    // its (absent) elements; it borrows the `: List<T>`
                    // annotation instead. Every other initializer is checked
                    // bottom-up as usual.
                    let inferred = match (value, ty) {
                        (Expr::ListLit { elems, span }, _) if elems.is_empty() => {
                            match ty.as_ref().and_then(|t| self.resolve_typeref(t)) {
                                Some(lt @ Ty::List(_)) => lt,
                                _ => {
                                    return Err(self.diag(
                                        DiagCode::Typ001,
                                        format!(
                                            "binding `{}` is an empty list `[]` and needs a `: List<T>` annotation",
                                            name
                                        ),
                                        "a `: List<T>` annotation",
                                        "[]".to_string(),
                                        *span,
                                    ))
                                }
                            }
                        }
                        _ => self.check_expr(value, &locals, uses, caller_name)?,
                    };
                    if let Some(t) = ty {
                        if let Some(declared) = self.resolve_typeref(t) {
                            if declared != inferred {
                                return Err(self.diag(
                                    DiagCode::Typ001,
                                    format!(
                                        "binding `{}` annotated {} but initialised with {}",
                                        name,
                                        declared.name(),
                                        inferred.name()
                                    ),
                                    declared.name(),
                                    inferred.name().to_string(),
                                    value.span(),
                                ));
                            }
                        }
                    }
                    locals.insert(name.clone(), (inferred, *mutable));
                }
                Stmt::Assign { name, value, span } => {
                    let (ty, mutable) = locals.get(name).copied().ok_or_else(|| {
                        self.diag(
                            DiagCode::Ref001,
                            format!("assignment to unknown variable `{}`", name),
                            "a variable in scope",
                            name.clone(),
                            *span,
                        )
                    })?;
                    if !mutable {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!("`{}` is not mutable; declare it `let mut {}`", name, name),
                            "a `let mut` binding",
                            format!("immutable `{}`", name),
                            *span,
                        ));
                    }
                    let got = self.check_expr(value, &locals, uses, caller_name)?;
                    if got != ty {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!(
                                "cannot assign {} to `{}` of type {}",
                                got.name(),
                                name,
                                ty.name()
                            ),
                            ty.name(),
                            got.name().to_string(),
                            value.span(),
                        ));
                    }
                }
                Stmt::FieldAssign { target, value, span } => {
                    // The lvalue is a field chain (`p.x`, `p.a.b`). Its root
                    // must be a `let mut` struct binding; the field type fixes
                    // the value type. Reference semantics mean the write goes
                    // through the heap handle (codegen lowers `... ->field = v`).
                    let field_ty =
                        self.check_field_lvalue(target, &locals, uses, caller_name, *span)?;
                    let got = self.check_expr(value, &locals, uses, caller_name)?;
                    if got != field_ty {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!(
                                "cannot assign {} to a field of type {}",
                                got.name(),
                                field_ty.name()
                            ),
                            field_ty.name(),
                            got.name().to_string(),
                            value.span(),
                        ));
                    }
                }
                Stmt::While { cond, body, span } => {
                    let ct = self.check_expr(cond, &locals, uses, caller_name)?;
                    if ct != Ty::Bool {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!("`while` condition must be Bool, got {}", ct.name()),
                            "Bool",
                            ct.name().to_string(),
                            *span,
                        ));
                    }
                    // The body runs for effect; check it in a child scope.
                    self.check_block(body, &locals, uses, caller_name)?;
                }
                Stmt::Expr(e) => {
                    self.check_expr(e, &locals, uses, caller_name)?;
                }
                Stmt::Return { value, .. } => {
                    if let Some(e) = value {
                        self.check_expr(e, &locals, uses, caller_name)?;
                    }
                }
            }
        }
        match &block.tail {
            Some(e) => self.check_expr(e, &locals, uses, caller_name),
            None => Ok(Ty::Unit),
        }
    }

    fn check_expr(
        &self,
        e: &Expr,
        locals: &HashMap<String, (Ty, bool)>,
        caller_uses: &HashSet<String>,
        caller_name: &str,
    ) -> Result<Ty, Diag> {
        match e {
            Expr::Str { .. } => Ok(Ty::String),
            Expr::Int { .. } => Ok(Ty::Int),
            Expr::Bool { .. } => Ok(Ty::Bool),
            Expr::Ident { name, span } => locals.get(name).map(|(t, _)| *t).ok_or_else(|| {
                self.diag(
                    DiagCode::Ref001,
                    format!("unknown identifier `{}`", name),
                    "a parameter or `let` binding in scope",
                    name.clone(),
                    *span,
                )
            }),
            Expr::Symbol { name, arg, span } => {
                let (id, idx) = match self.symbols.get(name) {
                    Some(v) => *v,
                    None => {
                        return Err(self.diag(
                            DiagCode::Ref010,
                            format!("unknown enum symbol `.{}`", name),
                            "a symbol from a declared enum",
                            format!(".{}", name),
                            *span,
                        ))
                    }
                };
                let variant = &self.enums[id as usize].variants[idx];
                match (&variant.payload, arg) {
                    (None, None) => Ok(Ty::Enum(id)),
                    (Some(pty), Some(e)) => {
                        let got = self.check_expr(e, locals, caller_uses, caller_name)?;
                        if got != *pty {
                            return Err(self.diag(
                                DiagCode::Typ001,
                                format!(
                                    "payload of `.{}` expects {}, got {}",
                                    name,
                                    pty.name(),
                                    got.name()
                                ),
                                pty.name(),
                                got.name().to_string(),
                                e.span(),
                            ));
                        }
                        Ok(Ty::Enum(id))
                    }
                    (Some(pty), None) => Err(self
                        .diag(
                            DiagCode::Typ021,
                            format!(
                                "`.{}` carries a {} payload but was constructed without one",
                                name,
                                pty.name()
                            ),
                            &format!(".{}(<{}>)", name, pty.name()),
                            format!(".{}", name),
                            *span,
                        )
                        .with_repair(
                            "supply-payload",
                            serde_json::json!({ "symbol": name, "ty": pty.name() }),
                        )),
                    (None, Some(_)) => Err(self
                        .diag(
                            DiagCode::Typ021,
                            format!("`.{}` carries no payload but was given one", name),
                            &format!(".{}", name),
                            format!(".{}(…)", name),
                            *span,
                        )
                        .with_repair(
                            "drop-payload",
                            serde_json::json!({ "symbol": name }),
                        )),
                }
            }
            Expr::Call { callee, args, span } => {
                // List intrinsics (`push`/`set`/`list_len`) are polymorphic in
                // the element type, so they cannot live in the monomorphic
                // SigTable — they are checked by hand (SPEC §11). These names
                // are reserved: a user `fn push` would be shadowed here.
                if let Some(t) =
                    self.check_list_intrinsic(callee, args, locals, caller_uses, caller_name, *span)?
                {
                    return Ok(t);
                }
                let sig = self.sigs.get(callee).ok_or_else(|| {
                    self.diag(
                        DiagCode::Ref001,
                        format!("unknown function `{}`", callee),
                        "a declared function or builtin",
                        callee.clone(),
                        *span,
                    )
                })?;

                if args.len() != sig.params.len() {
                    return Err(self.diag(
                        DiagCode::Typ002,
                        format!(
                            "`{}` expects {} argument(s), got {}",
                            callee,
                            sig.params.len(),
                            args.len()
                        ),
                        &format!("{} argument(s)", sig.params.len()),
                        format!("{} argument(s)", args.len()),
                        *span,
                    ));
                }

                for (arg, expected) in args.iter().zip(sig.params.iter()) {
                    let got = self.check_expr(arg, locals, caller_uses, caller_name)?;
                    if got != *expected {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!(
                                "argument to `{}` has wrong type: expected {}, got {}",
                                callee,
                                expected.name(),
                                got.name()
                            ),
                            expected.name(),
                            got.name().to_string(),
                            arg.span(),
                        ));
                    }
                }

                // Effect discipline (SPEC §5): callee effects ⊆ caller `uses`.
                for eff in &sig.effects {
                    if !caller_uses.contains(eff) {
                        return Err(self
                            .diag(
                                DiagCode::Eff001,
                                format!(
                                    "`{}` exercises capability `!{}` but `{}` does not declare it",
                                    callee, eff, caller_name
                                ),
                                &format!("`{}` declares `uses !{}`", caller_name, eff),
                                format!("uses without !{}", eff),
                                *span,
                            )
                            .with_repair(
                                "add-uses-clause",
                                serde_json::json!({ "fn": caller_name, "cap": eff }),
                            ));
                    }
                }

                Ok(sig.ret)
            }
            Expr::Unary { op: UnOp::Neg, operand, span } => {
                let t = self.check_expr(operand, locals, caller_uses, caller_name)?;
                if t != Ty::Int {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        format!("unary `-` expects Int, got {}", t.name()),
                        "Int",
                        t.name().to_string(),
                        *span,
                    ));
                }
                Ok(Ty::Int)
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lt = self.check_expr(lhs, locals, caller_uses, caller_name)?;
                let rt = self.check_expr(rhs, locals, caller_uses, caller_name)?;
                self.check_binary(*op, lt, rt, *span)
            }
            Expr::If { cond, then_blk, else_blk, span } => {
                let ct = self.check_expr(cond, locals, caller_uses, caller_name)?;
                if ct != Ty::Bool {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        format!("`if` condition must be Bool, got {}", ct.name()),
                        "Bool",
                        ct.name().to_string(),
                        cond.span(),
                    ));
                }
                let tt = self.check_block(then_blk, locals, caller_uses, caller_name)?;
                let et = self.check_block(else_blk, locals, caller_uses, caller_name)?;
                if tt != et {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        format!(
                            "`if` branches have different types: {} vs {}",
                            tt.name(),
                            et.name()
                        ),
                        tt.name(),
                        et.name().to_string(),
                        *span,
                    ));
                }
                Ok(tt)
            }
            Expr::Match { scrutinee, arms, span } => {
                let st = self.check_expr(scrutinee, locals, caller_uses, caller_name)?;
                let id = match st {
                    Ty::Enum(id) => id,
                    other => {
                        return Err(self.diag(
                            DiagCode::Typ020,
                            format!("`match` scrutinee must be an enum, got {}", other.name()),
                            "an enum value",
                            other.name().to_string(),
                            scrutinee.span(),
                        ))
                    }
                };
                let edef = &self.enums[id as usize];

                let mut covered: HashSet<String> = HashSet::new();
                let mut arm_ty: Option<Ty> = None;
                for arm in arms {
                    let vidx = match edef.variants.iter().position(|v| v.name == arm.symbol) {
                        Some(i) => i,
                        None => {
                            return Err(self.diag(
                                DiagCode::Typ020,
                                format!(
                                    "`.{}` is not a variant of enum `{}`",
                                    arm.symbol, edef.name
                                ),
                                &format!("one of {}", symbol_list(edef)),
                                format!(".{}", arm.symbol),
                                arm.span,
                            ))
                        }
                    };
                    if !covered.insert(arm.symbol.clone()) {
                        return Err(self.diag(
                            DiagCode::Typ020,
                            format!("duplicate match arm `.{}`", arm.symbol),
                            "each arm at most once",
                            format!(".{}", arm.symbol),
                            arm.span,
                        ));
                    }
                    // The payload binder must match the variant's payload
                    // arity; a bound payload is in scope in the arm body.
                    let mut arm_locals = locals.clone();
                    match (&edef.variants[vidx].payload, &arm.binder) {
                        (None, None) => {}
                        (Some(pty), Some(b)) => {
                            if b != "_" {
                                arm_locals.insert(b.clone(), (*pty, false));
                            }
                        }
                        (Some(pty), None) => {
                            return Err(self
                                .diag(
                                    DiagCode::Typ021,
                                    format!(
                                        "arm `.{}` must bind its {} payload",
                                        arm.symbol,
                                        pty.name()
                                    ),
                                    &format!(".{}(<name>) =>", arm.symbol),
                                    format!(".{} =>", arm.symbol),
                                    arm.span,
                                )
                                .with_repair(
                                    "bind-payload",
                                    serde_json::json!({ "symbol": arm.symbol, "ty": pty.name() }),
                                ))
                        }
                        (None, Some(_)) => {
                            return Err(self.diag(
                                DiagCode::Typ021,
                                format!(
                                    "arm `.{}` binds a payload but the variant carries none",
                                    arm.symbol
                                ),
                                &format!(".{} =>", arm.symbol),
                                format!(".{}(…) =>", arm.symbol),
                                arm.span,
                            ))
                        }
                    }
                    let bt = self.check_expr(&arm.body, &arm_locals, caller_uses, caller_name)?;
                    match arm_ty {
                        None => arm_ty = Some(bt),
                        Some(prev) if prev != bt => {
                            return Err(self.diag(
                                DiagCode::Typ001,
                                format!(
                                    "match arms have different types: {} vs {}",
                                    prev.name(),
                                    bt.name()
                                ),
                                prev.name(),
                                bt.name().to_string(),
                                arm.span,
                            ))
                        }
                        _ => {}
                    }
                }

                let missing: Vec<String> = edef
                    .variants
                    .iter()
                    .map(|v| v.name.clone())
                    .filter(|s| !covered.contains(s))
                    .collect();
                if !missing.is_empty() {
                    return Err(self
                        .diag(
                            DiagCode::Typ020,
                            format!(
                                "non-exhaustive match on `{}`: missing {}",
                                edef.name,
                                missing
                                    .iter()
                                    .map(|s| format!(".{}", s))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            &format!("arms for all of {}", symbol_list(edef)),
                            format!("missing {} arm(s)", missing.len()),
                            *span,
                        )
                        .with_repair(
                            "add-missing-arm",
                            serde_json::json!({ "enum": edef.name, "symbol": missing[0] }),
                        ));
                }

                Ok(arm_ty.unwrap_or(Ty::Unit))
            }
            Expr::ListLit { elems, span } => {
                // `[]` is only well-typed where an annotation supplies the
                // element type; the `let` handler catches that case before it
                // ever reaches here, so a bare empty literal is an error.
                if elems.is_empty() {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        "empty list literal `[]` needs a `: List<T>` annotation to fix its element type".to_string(),
                        "a `let xs: List<T> = []` annotation",
                        "[]".to_string(),
                        *span,
                    ));
                }
                let first = self.check_expr(&elems[0], locals, caller_uses, caller_name)?;
                let elem = ElemTy::from_ty(first).ok_or_else(|| {
                    self.diag(
                        DiagCode::Typ001,
                        format!(
                            "list elements must be Int, Bool, or String, got {}",
                            first.name()
                        ),
                        "Int, Bool, or String",
                        first.name().to_string(),
                        elems[0].span(),
                    )
                })?;
                for e in &elems[1..] {
                    let t = self.check_expr(e, locals, caller_uses, caller_name)?;
                    if t != first {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!(
                                "list elements must share one type: expected {}, got {}",
                                first.name(),
                                t.name()
                            ),
                            first.name(),
                            t.name().to_string(),
                            e.span(),
                        ));
                    }
                }
                Ok(Ty::List(elem))
            }
            Expr::Index { base, index, span } => {
                let bt = self.check_expr(base, locals, caller_uses, caller_name)?;
                let elem = match bt {
                    Ty::List(e) => e,
                    other => {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!("cannot index {}; only a List can be indexed", other.name()),
                            "a List value",
                            other.name().to_string(),
                            *span,
                        ))
                    }
                };
                let it = self.check_expr(index, locals, caller_uses, caller_name)?;
                if it != Ty::Int {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        format!("list index must be Int, got {}", it.name()),
                        "Int",
                        it.name().to_string(),
                        index.span(),
                    ));
                }
                Ok(elem.as_ty())
            }
            Expr::StructLit { name, fields, span } => {
                let id = self.struct_by_name(name).ok_or_else(|| {
                    self.diag(
                        DiagCode::Ref001,
                        format!("unknown struct `{}`", name),
                        "a declared struct type",
                        name.clone(),
                        *span,
                    )
                })?;
                let sinfo = &self.structs[id as usize];
                // Every field exactly once, no unknowns, types match.
                let mut seen: HashSet<String> = HashSet::new();
                for fi in fields {
                    let (_, expected) = sinfo.field(&fi.name).ok_or_else(|| {
                        self.diag(
                            DiagCode::Ref001,
                            format!("struct `{}` has no field `{}`", name, fi.name),
                            "a declared field",
                            fi.name.clone(),
                            fi.span,
                        )
                    })?;
                    if !seen.insert(fi.name.clone()) {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!("field `{}` set twice in `{}` literal", fi.name, name),
                            "each field at most once",
                            fi.name.clone(),
                            fi.span,
                        ));
                    }
                    let got = self.check_expr(&fi.value, locals, caller_uses, caller_name)?;
                    if got != expected {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!(
                                "field `{}` expects {}, got {}",
                                fi.name,
                                expected.name(),
                                got.name()
                            ),
                            expected.name(),
                            got.name().to_string(),
                            fi.value.span(),
                        ));
                    }
                }
                let missing: Vec<String> = sinfo
                    .fields
                    .iter()
                    .map(|(n, _)| n.clone())
                    .filter(|n| !seen.contains(n))
                    .collect();
                if !missing.is_empty() {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        format!(
                            "`{}` literal is missing field(s): {}",
                            name,
                            missing.join(", ")
                        ),
                        &format!("all fields of `{}`", name),
                        format!("missing {}", missing.len()),
                        *span,
                    ));
                }
                Ok(Ty::Struct(id))
            }
            Expr::Field { base, field, span } => {
                let bt = self.check_expr(base, locals, caller_uses, caller_name)?;
                let id = match bt {
                    Ty::Struct(id) => id,
                    other => {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!("cannot read field `.{}` of {}; only a struct has fields", field, other.name()),
                            "a struct value",
                            other.name().to_string(),
                            *span,
                        ))
                    }
                };
                let sinfo = &self.structs[id as usize];
                let (_, fty) = sinfo.field(field).ok_or_else(|| {
                    self.diag(
                        DiagCode::Ref001,
                        format!("struct `{}` has no field `{}`", sinfo.name, field),
                        "a declared field",
                        field.clone(),
                        *span,
                    )
                })?;
                Ok(fty)
            }
        }
    }

    fn struct_by_name(&self, name: &str) -> Option<u32> {
        // Small linear scan: struct tables are tiny. The id is the table index.
        self.structs.iter().position(|s| s.name == name).map(|i| i as u32)
    }

    /// Validate a field-assignment lvalue (`p.x`, `p.a.b`): the chain must be a
    /// `.field` path rooted at a `let mut` struct binding. Returns the type of
    /// the final field (what the assigned value must match).
    fn check_field_lvalue(
        &self,
        target: &Expr,
        locals: &HashMap<String, (Ty, bool)>,
        caller_uses: &HashSet<String>,
        caller_name: &str,
        span: Span,
    ) -> Result<Ty, Diag> {
        // The outermost node must be a field access.
        let (base, field) = match target {
            Expr::Field { base, field, .. } => (base, field),
            _ => {
                return Err(self.diag(
                    DiagCode::Typ001,
                    "assignment target must be a struct field".to_string(),
                    "a `p.field` lvalue",
                    "an expression".to_string(),
                    span,
                ))
            }
        };
        // The root of the chain must be a mutable binding; intermediate hops
        // are field reads (reference semantics — the handle is reachable).
        self.require_mut_root(target, locals, span)?;
        // Type the base (a struct) then resolve the written field.
        let bt = self.check_expr(base, locals, caller_uses, caller_name)?;
        let id = match bt {
            Ty::Struct(id) => id,
            other => {
                return Err(self.diag(
                    DiagCode::Typ001,
                    format!("cannot write field `.{}` of {}; only a struct has fields", field, other.name()),
                    "a struct value",
                    other.name().to_string(),
                    span,
                ))
            }
        };
        let sinfo = &self.structs[id as usize];
        let (_, fty) = sinfo.field(field).ok_or_else(|| {
            self.diag(
                DiagCode::Ref001,
                format!("struct `{}` has no field `{}`", sinfo.name, field),
                "a declared field",
                field.clone(),
                span,
            )
        })?;
        Ok(fty)
    }

    /// Walk a field-access chain down to its root identifier and require that
    /// binding to be `let mut` (the surface mutability discipline, mirroring
    /// `push`/`set` on lists).
    fn require_mut_root(
        &self,
        e: &Expr,
        locals: &HashMap<String, (Ty, bool)>,
        span: Span,
    ) -> Result<(), Diag> {
        match e {
            Expr::Field { base, .. } => self.require_mut_root(base, locals, span),
            Expr::Ident { name, .. } => {
                let (_, mutable) = locals.get(name).copied().ok_or_else(|| {
                    self.diag(
                        DiagCode::Ref001,
                        format!("assignment to unknown variable `{}`", name),
                        "a variable in scope",
                        name.clone(),
                        span,
                    )
                })?;
                if !mutable {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        format!("`{}` is not mutable; declare it `let mut {}`", name, name),
                        "a `let mut` binding",
                        format!("immutable `{}`", name),
                        span,
                    ));
                }
                Ok(())
            }
            _ => Err(self.diag(
                DiagCode::Typ001,
                "field assignment must be rooted at a variable".to_string(),
                "a `let mut` variable at the root",
                "an expression".to_string(),
                span,
            )),
        }
    }

    /// Check a list intrinsic call (`push`/`set`/`list_len`). Returns
    /// `Ok(Some(ty))` if `callee` names an intrinsic, `Ok(None)` otherwise so
    /// the normal call path runs. `push`/`set` mutate in place, so their list
    /// argument must be a `let mut` binding (SPEC §11 — the surface mutability
    /// rule even though the C handle has reference semantics).
    fn check_list_intrinsic(
        &self,
        callee: &str,
        args: &[Expr],
        locals: &HashMap<String, (Ty, bool)>,
        caller_uses: &HashSet<String>,
        caller_name: &str,
        span: Span,
    ) -> Result<Option<Ty>, Diag> {
        match callee {
            "list_len" => {
                if args.len() != 1 {
                    return Err(self.arity_diag(callee, 1, args.len(), span));
                }
                let bt = self.check_expr(&args[0], locals, caller_uses, caller_name)?;
                match bt {
                    Ty::List(_) => Ok(Some(Ty::Int)),
                    other => Err(self.diag(
                        DiagCode::Typ001,
                        format!("`list_len` expects a List, got {}", other.name()),
                        "a List value",
                        other.name().to_string(),
                        args[0].span(),
                    )),
                }
            }
            "push" | "set" => {
                let want = if callee == "push" { 2 } else { 3 };
                if args.len() != want {
                    return Err(self.arity_diag(callee, want, args.len(), span));
                }
                // The target must be a named, mutable list binding.
                let elem = self.mutable_list_arg(callee, &args[0], locals)?;
                let val_idx = want - 1;
                if callee == "set" {
                    let it = self.check_expr(&args[1], locals, caller_uses, caller_name)?;
                    if it != Ty::Int {
                        return Err(self.diag(
                            DiagCode::Typ001,
                            format!("`set` index must be Int, got {}", it.name()),
                            "Int",
                            it.name().to_string(),
                            args[1].span(),
                        ));
                    }
                }
                let vt = self.check_expr(&args[val_idx], locals, caller_uses, caller_name)?;
                if vt != elem.as_ty() {
                    return Err(self.diag(
                        DiagCode::Typ001,
                        format!(
                            "`{}` value has type {} but the list holds {}",
                            callee,
                            vt.name(),
                            elem.name()
                        ),
                        elem.name(),
                        vt.name().to_string(),
                        args[val_idx].span(),
                    ));
                }
                Ok(Some(Ty::Unit))
            }
            _ => Ok(None),
        }
    }

    fn arity_diag(&self, callee: &str, want: usize, got: usize, span: Span) -> Diag {
        self.diag(
            DiagCode::Typ002,
            format!("`{}` expects {} argument(s), got {}", callee, want, got),
            &format!("{} argument(s)", want),
            format!("{} argument(s)", got),
            span,
        )
    }

    /// Resolve the element type of a `push`/`set` target, requiring it to be a
    /// `let mut` list binding by name.
    fn mutable_list_arg(
        &self,
        callee: &str,
        arg: &Expr,
        locals: &HashMap<String, (Ty, bool)>,
    ) -> Result<ElemTy, Diag> {
        let (name, span) = match arg {
            Expr::Ident { name, span } => (name, *span),
            other => {
                return Err(self.diag(
                    DiagCode::Typ001,
                    format!("`{}` needs a named mutable list as its first argument", callee),
                    "a `let mut` list binding",
                    "an expression".to_string(),
                    other.span(),
                ))
            }
        };
        let (ty, mutable) = locals.get(name).copied().ok_or_else(|| {
            self.diag(
                DiagCode::Ref001,
                format!("unknown variable `{}`", name),
                "a variable in scope",
                name.clone(),
                span,
            )
        })?;
        let elem = match ty {
            Ty::List(e) => e,
            other => {
                return Err(self.diag(
                    DiagCode::Typ001,
                    format!("`{}` expects a List, but `{}` is {}", callee, name, other.name()),
                    "a List value",
                    other.name().to_string(),
                    span,
                ))
            }
        };
        if !mutable {
            return Err(self.diag(
                DiagCode::Typ001,
                format!("`{}` mutates `{}`; declare it `let mut {}`", callee, name, name),
                "a `let mut` binding",
                format!("immutable `{}`", name),
                span,
            ));
        }
        Ok(elem)
    }

    /// Type rule for a binary operator given its operand types.
    fn check_binary(&self, op: BinOp, lt: Ty, rt: Ty, span: Span) -> Result<Ty, Diag> {
        let bad = |want: &str, got: Ty, sp: Span, this: &Self| {
            this.diag(
                DiagCode::Typ001,
                format!("operator `{}` expects {} operands, got {}", op.c_op(), want, got.name()),
                want,
                got.name().to_string(),
                sp,
            )
        };
        if op.is_arithmetic() {
            if lt != Ty::Int {
                return Err(bad("Int", lt, span, self));
            }
            if rt != Ty::Int {
                return Err(bad("Int", rt, span, self));
            }
            Ok(Ty::Int)
        } else if op.is_ordering() {
            if lt != Ty::Int || rt != Ty::Int {
                return Err(bad("Int", if lt != Ty::Int { lt } else { rt }, span, self));
            }
            Ok(Ty::Bool)
        } else if op.is_equality() {
            // Tier 0: String equality (lowered to strcmp in codegen) joins
            // Int/Bool. Needed to recognise commands and keystrokes.
            if lt != rt || !matches!(lt, Ty::Int | Ty::Bool | Ty::String) {
                return Err(self.diag(
                    DiagCode::Typ001,
                    format!(
                        "operator `{}` compares two Int, two Bool, or two String, got {} and {}",
                        op.c_op(),
                        lt.name(),
                        rt.name()
                    ),
                    "matching Int, Bool, or String operands",
                    format!("{} and {}", lt.name(), rt.name()),
                    span,
                ));
            }
            Ok(Ty::Bool)
        } else {
            if lt != Ty::Bool || rt != Ty::Bool {
                return Err(bad("Bool", if lt != Ty::Bool { lt } else { rt }, span, self));
            }
            Ok(Ty::Bool)
        }
    }
}

fn symbol_list(e: &EnumInfo) -> String {
    e.variants
        .iter()
        .map(|v| format!(".{}", v.name))
        .collect::<Vec<_>>()
        .join(", ")
}
