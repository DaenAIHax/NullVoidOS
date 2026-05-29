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

/// Output of checking: signatures, the enum-symbol → (enum id, variant
/// index) map, and the resolved enum table. Codegen lowers a payload-free
/// symbol to its index; a payload-carrying enum lowers to a tagged union.
pub struct Checked {
    pub sigs: SigTable,
    pub symbols: HashMap<String, (u32, usize)>,
    pub enums: Vec<EnumInfo>,
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

    // Pass 1: collect user function signatures.
    let mut sigs = builtins();
    for item in &file.items {
        if let Item::Func(f) = item {
            let mut params = Vec::new();
            for p in &f.params {
                params.push(resolve_ty(&p.ty, &enum_by_name, src, fname)?);
            }
            let ret = resolve_ty(&f.ret, &enum_by_name, src, fname)?;
            let effects = f.uses.iter().map(|c| c.key()).collect();
            sigs.insert(
                f.name.clone(),
                Sig {
                    params,
                    ret,
                    effects,
                    c_name: f.name.clone(),
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
    src: &str,
    fname: &str,
) -> Result<Ty, Diag> {
    if let Some(ty) = t.resolved {
        return Ok(ty);
    }
    if let Some(id) = enum_by_name.get(&t.name) {
        return Ok(Ty::Enum(*id));
    }
    let (line, col) = line_col(src, t.span.offset);
    Err(Diag::error(
        DiagCode::Typ003,
        format!("unknown type `{}`", t.name),
        "Int, Bool, String, Unit, World, or a declared enum",
        t.name.clone(),
        fname,
        line,
        col,
    ))
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
}

impl<'a> Checker<'a> {
    fn diag(&self, code: DiagCode, msg: String, expected: &str, actual: String, span: Span) -> Diag {
        let (line, col) = line_col(self.src, span.offset);
        Diag::error(code, msg, expected, actual, self.fname, line, col)
    }

    fn check_func(&self, f: &Func) -> Result<(), Diag> {
        let sig = self.sigs.get(&f.name).expect("signature collected in pass 1");
        let mut locals: HashMap<String, Ty> = HashMap::new();
        for (p, ty) in f.params.iter().zip(sig.params.iter()) {
            locals.insert(p.name.clone(), *ty);
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
        outer: &HashMap<String, Ty>,
        uses: &HashSet<String>,
        caller_name: &str,
    ) -> Result<Ty, Diag> {
        let mut locals = outer.clone();
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let { name, ty, value, .. } => {
                    let inferred = self.check_expr(value, &locals, uses, caller_name)?;
                    if let Some(t) = ty {
                        if let Some(declared) = t.resolved {
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
                    locals.insert(name.clone(), inferred);
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
        locals: &HashMap<String, Ty>,
        caller_uses: &HashSet<String>,
        caller_name: &str,
    ) -> Result<Ty, Diag> {
        match e {
            Expr::Str { .. } => Ok(Ty::String),
            Expr::Int { .. } => Ok(Ty::Int),
            Expr::Bool { .. } => Ok(Ty::Bool),
            Expr::Ident { name, span } => locals.get(name).copied().ok_or_else(|| {
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
                                arm_locals.insert(b.clone(), *pty);
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
        }
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
