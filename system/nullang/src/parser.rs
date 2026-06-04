//! Recursive-descent parser for Nullang's construction core (v0.1).
//!
//! Grammar (minimal, SPEC §4):
//!   file   := func*
//!   func   := 'fn' ident '(' params? ')' '->' type uses? block
//!   params := param (',' param)*
//!   param  := ident ':' type
//!   uses   := 'uses' capability (',' capability)*
//!   block  := '{' stmt* expr? '}'
//!   stmt   := 'let' ident (':' type)? '=' expr ';'
//!           | 'return' expr? ';'
//!           | expr ';'
//!   expr   := literal | ident | ident '(' args? ')'
//!   cap    := '!' ident ('.' ident)* ('.' string)?
use crate::ast::*;
use crate::diagnostics::{Diag, DiagCode};
use crate::lexer::{line_col, Token, TokenKind};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    src: &'a str,
    file: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, src: &'a str, file: &'a str) -> Self {
        Parser {
            tokens,
            pos: 0,
            src,
            file,
        }
    }

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_offset(&self) -> usize {
        self.tokens[self.pos].offset
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn span_here(&self) -> Span {
        Span {
            offset: self.peek_offset(),
        }
    }

    fn err(&self, code: DiagCode, message: impl Into<String>, expected: impl Into<String>) -> Diag {
        let (line, col) = line_col(self.src, self.peek_offset());
        Diag::error(
            code,
            message,
            expected,
            format!("{}", self.peek()),
            self.file,
            line,
            col,
        )
    }

    fn expect(&mut self, want: &TokenKind, ctx: &str) -> Result<Token, Diag> {
        if self.peek() == want {
            Ok(self.advance())
        } else {
            Err(self.err(
                DiagCode::Par010,
                format!("expected `{}` {}, found `{}`", want, ctx, self.peek()),
                format!("{}", want),
            ))
        }
    }

    fn expect_ident(&mut self, ctx: &str) -> Result<(String, Span), Diag> {
        let span = self.span_here();
        match self.peek().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok((name, span))
            }
            other => Err(self.err(
                DiagCode::Par010,
                format!("expected identifier {}, found `{}`", ctx, other),
                "identifier",
            )),
        }
    }

    pub fn parse_file(&mut self) -> Result<File, Diag> {
        let mut items = Vec::new();
        while *self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::Fn => items.push(Item::Func(self.parse_func()?)),
                TokenKind::Enum => items.push(Item::Enum(self.parse_enum()?)),
                TokenKind::Type => items.push(Item::Struct(self.parse_struct()?)),
                other => {
                    return Err(self.err(
                        DiagCode::Par010,
                        format!("expected `fn`, `enum`, or `type` at top level, found `{}`", other),
                        "fn, enum, or type",
                    ))
                }
            }
        }
        Ok(File { items })
    }

    fn parse_enum(&mut self) -> Result<EnumDef, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::Enum, "to start an enum")?;
        let (name, _) = self.expect_ident("for the enum name")?;
        self.expect(&TokenKind::Eq, "after the enum name")?;
        let mut variants = Vec::new();
        loop {
            let vspan = self.span_here();
            self.expect(&TokenKind::Dot, "before an enum symbol")?;
            let (sym, _) = self.expect_ident("for an enum symbol")?;
            // Optional single typed payload: `.ok(Int)`.
            let payload = if *self.peek() == TokenKind::LParen {
                self.advance();
                let ty = self.parse_type()?;
                self.expect(&TokenKind::RParen, "to close an enum payload type")?;
                Some(ty)
            } else {
                None
            };
            variants.push(Variant {
                name: sym,
                payload,
                span: vspan,
            });
            if *self.peek() == TokenKind::Pipe {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(&TokenKind::Semi, "to end the enum declaration")?;
        Ok(EnumDef {
            name,
            variants,
            span,
        })
    }

    /// `type Name = { field: Type, ... };` — a nominal struct (SPEC §11, v0.4).
    fn parse_struct(&mut self) -> Result<StructDef, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::Type, "to start a struct declaration")?;
        let (name, _) = self.expect_ident("for the struct name")?;
        self.expect(&TokenKind::Eq, "after the struct name")?;
        self.expect(&TokenKind::LBrace, "to open the field list")?;
        let mut fields = Vec::new();
        if *self.peek() != TokenKind::RBrace {
            loop {
                let fspan = self.span_here();
                let (fname, _) = self.expect_ident("for a field name")?;
                self.expect(&TokenKind::Colon, "after the field name")?;
                let ty = self.parse_type()?;
                fields.push(FieldDef { name: fname, ty, span: fspan });
                if *self.peek() == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RBrace, "to close the field list")?;
        self.expect(&TokenKind::Semi, "to end the struct declaration")?;
        Ok(StructDef { name, fields, span })
    }

    fn parse_func(&mut self) -> Result<Func, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::Fn, "to start a function")?;
        let (name, _) = self.expect_ident("for the function name")?;
        self.expect(&TokenKind::LParen, "after the function name")?;

        let mut params = Vec::new();
        if *self.peek() != TokenKind::RParen {
            loop {
                params.push(self.parse_param()?);
                if *self.peek() == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "to close the parameter list")?;
        self.expect(&TokenKind::Arrow, "before the return type")?;
        let ret = self.parse_type()?;

        let mut uses = Vec::new();
        if *self.peek() == TokenKind::Uses {
            self.advance();
            loop {
                uses.push(self.parse_capability()?);
                if *self.peek() == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        let body = self.parse_block()?;
        Ok(Func {
            name,
            params,
            ret,
            uses,
            body,
            span,
        })
    }

    fn parse_param(&mut self) -> Result<Param, Diag> {
        let span = self.span_here();
        let (name, _) = self.expect_ident("for a parameter name")?;
        self.expect(&TokenKind::Colon, "after the parameter name")?;
        let ty = self.parse_type()?;
        Ok(Param { name, ty, span })
    }

    fn parse_type(&mut self) -> Result<TypeRef, Diag> {
        let span = self.span_here();
        // `()` is Unit (SPEC §4.2).
        if *self.peek() == TokenKind::LParen {
            self.advance();
            self.expect(&TokenKind::RParen, "to form the Unit type `()`")?;
            return Ok(TypeRef {
                resolved: Some(Ty::Unit),
                name: "()".to_string(),
                span,
                elem: None,
            });
        }
        let (name, _) = self.expect_ident("for a type")?;
        // `List<T>` — a built-in monomorphic container (SPEC §11, v0.3/v0.4).
        // Element types are scalar (Int/Bool/String) or a struct (v0.4). A
        // scalar resolves here; a struct name does not (the parser has no
        // struct table), so its element `TypeRef` is stashed in `elem` for the
        // checker to finish. Nested lists / lists of enums are deferred.
        if name == "List" && *self.peek() == TokenKind::Lt {
            self.advance(); // `<`
            let elem = self.parse_type()?;
            self.expect(&TokenKind::Gt, "to close the `List<...>` element type")?;
            let resolved = elem.resolved.and_then(ElemTy::from_ty).map(Ty::List);
            return Ok(TypeRef {
                resolved,
                name: format!("List<{}>", elem.name),
                span,
                elem: Some(Box::new(elem)),
            });
        }
        let resolved = match name.as_str() {
            "Int" => Some(Ty::Int),
            "Bool" => Some(Ty::Bool),
            "String" => Some(Ty::String),
            "Unit" => Some(Ty::Unit),
            "World" => Some(Ty::World),
            _ => None,
        };
        Ok(TypeRef {
            resolved,
            name,
            span,
            elem: None,
        })
    }

    fn parse_capability(&mut self) -> Result<Capability, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::Bang, "to start a capability")?;
        let mut path = Vec::new();
        let (first, _) = self.expect_ident("for a capability root")?;
        path.push(first);
        let mut arg = None;
        while *self.peek() == TokenKind::Dot {
            self.advance();
            match self.peek().clone() {
                TokenKind::Ident(seg) => {
                    self.advance();
                    path.push(seg);
                }
                TokenKind::Str(s) => {
                    self.advance();
                    arg = Some(s);
                    break; // a string argument is always the final segment
                }
                other => {
                    return Err(self.err(
                        DiagCode::Par010,
                        format!("expected capability segment or string, found `{}`", other),
                        "identifier or string",
                    ))
                }
            }
        }
        Ok(Capability { path, arg, span })
    }

    fn parse_block(&mut self) -> Result<Block, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::LBrace, "to open a block")?;
        let mut stmts = Vec::new();
        let mut tail = None;

        while *self.peek() != TokenKind::RBrace && *self.peek() != TokenKind::Eof {
            match self.peek() {
                TokenKind::Let => stmts.push(self.parse_let()?),
                TokenKind::While => stmts.push(self.parse_while()?),
                TokenKind::Return => stmts.push(self.parse_return()?),
                _ => {
                    let expr = self.parse_expr()?;
                    if *self.peek() == TokenKind::Eq {
                        // Assignment. Two lvalue forms: a bare variable
                        // (`x = v`) and a struct field chain (`p.x = v`,
                        // `p.a.b = v`, SPEC §11). An index lvalue (`xs[i] = v`)
                        // is intentionally not a form — list writes go through
                        // `set` (§10, one way per concept).
                        let span = expr.span();
                        self.advance(); // `=`
                        let value = self.parse_expr()?;
                        self.expect(&TokenKind::Semi, "to end the assignment")?;
                        match expr {
                            Expr::Ident { name, .. } => {
                                stmts.push(Stmt::Assign { name, value, span });
                            }
                            field @ Expr::Field { .. } => {
                                stmts.push(Stmt::FieldAssign {
                                    target: Box::new(field),
                                    value,
                                    span,
                                });
                            }
                            _ => {
                                return Err(self.err(
                                    DiagCode::Par010,
                                    "only a variable or a struct field can be assigned (left of `=`)",
                                    "an identifier or a field access",
                                ))
                            }
                        }
                    } else if *self.peek() == TokenKind::Semi {
                        self.advance();
                        stmts.push(Stmt::Expr(expr));
                    } else if matches!(expr, Expr::If { .. } | Expr::Match { .. })
                        && *self.peek() != TokenKind::RBrace
                    {
                        // A block-like expression (`if`/`match`) that is *not*
                        // in tail position is a statement on its own — no `;`
                        // required, as in Rust. (When it *is* the last thing in
                        // the block, i.e. the next token is `}`, it falls
                        // through to the trailing-value case below and yields
                        // the block's value, preserving prior behaviour.)
                        stmts.push(Stmt::Expr(expr));
                    } else {
                        // No `;` → this is the block's trailing value.
                        tail = Some(expr);
                        break;
                    }
                }
            }
        }
        self.expect(&TokenKind::RBrace, "to close a block")?;
        Ok(Block { stmts, tail, span })
    }

    fn parse_let(&mut self) -> Result<Stmt, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::Let, "to start a binding")?;
        // Optional `mut`: a reassignable binding (SPEC §11).
        let mutable = if *self.peek() == TokenKind::Mut {
            self.advance();
            true
        } else {
            false
        };
        let (name, _) = self.expect_ident("for the binding name")?;
        let ty = if *self.peek() == TokenKind::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&TokenKind::Eq, "before the bound value")?;
        let value = self.parse_expr()?;
        self.expect(&TokenKind::Semi, "to end the binding")?;
        Ok(Stmt::Let {
            name,
            mutable,
            ty,
            value,
            span,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::While, "to start a loop")?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While { cond, body, span })
    }

    fn parse_return(&mut self) -> Result<Stmt, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::Return, "to start a return")?;
        let value = if *self.peek() == TokenKind::Semi {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&TokenKind::Semi, "to end the return")?;
        Ok(Stmt::Return { value, span })
    }

    // Expression grammar by precedence (lowest to highest):
    //   or → and → equality → comparison → additive → multiplicative
    //      → unary → primary
    fn parse_expr(&mut self) -> Result<Expr, Diag> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.parse_and()?;
        while *self.peek() == TokenKind::PipePipe {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = mk_binary(BinOp::Or, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.parse_equality()?;
        while *self.peek() == TokenKind::AmpAmp {
            self.advance();
            let rhs = self.parse_equality()?;
            lhs = mk_binary(BinOp::And, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                TokenKind::EqEq => BinOp::Eq,
                TokenKind::BangEq => BinOp::Ne,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_comparison()?;
            lhs = mk_binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_additive()?;
            lhs = mk_binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_multiplicative()?;
            lhs = mk_binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, Diag> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = mk_binary(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diag> {
        if *self.peek() == TokenKind::Minus {
            let span = self.span_here();
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnOp::Neg,
                operand: Box::new(expr),
                span,
            });
        }
        self.parse_postfix()
    }

    /// A primary followed by zero or more postfix operators (SPEC §11):
    /// `[index]` reads on a list and `.field` reads on a struct, chained
    /// freely (`p.next.value`, `xs[i]`). Tighter than unary `-`. A `.field`
    /// here is unambiguous against an enum symbol `.red`: the latter only
    /// appears at the *start* of a primary, never after a value expression.
    fn parse_postfix(&mut self) -> Result<Expr, Diag> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                TokenKind::LBracket => {
                    let span = expr.span();
                    self.advance(); // `[`
                    let index = self.parse_expr()?;
                    self.expect(&TokenKind::RBracket, "to close an index `[...]`")?;
                    expr = Expr::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                        span,
                    };
                }
                TokenKind::Dot => {
                    let span = expr.span();
                    self.advance(); // `.`
                    let (field, _) = self.expect_ident("for a struct field name")?;
                    expr = Expr::Field {
                        base: Box::new(expr),
                        field,
                        span,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Diag> {
        let span = self.span_here();
        match self.peek().clone() {
            TokenKind::Str(value) => {
                self.advance();
                Ok(Expr::Str { value, span })
            }
            TokenKind::Int(value) => {
                self.advance();
                Ok(Expr::Int { value, span })
            }
            TokenKind::Bool(value) => {
                self.advance();
                Ok(Expr::Bool { value, span })
            }
            TokenKind::LParen => {
                self.advance();
                let inner = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "to close a parenthesised expression")?;
                Ok(inner)
            }
            TokenKind::If => self.parse_if(),
            TokenKind::Match => self.parse_match(),
            TokenKind::LBracket => {
                // List literal `[a, b, c]` (SPEC §11). A trailing comma is not
                // accepted (one-way regularity, §10). `[]` is empty; its element
                // type comes from a surrounding `: List<T>` annotation.
                self.advance();
                let mut elems = Vec::new();
                if *self.peek() != TokenKind::RBracket {
                    loop {
                        elems.push(self.parse_expr()?);
                        if *self.peek() == TokenKind::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RBracket, "to close a list literal")?;
                Ok(Expr::ListLit { elems, span })
            }
            TokenKind::Dot => {
                self.advance();
                let (name, _) = self.expect_ident("for an enum symbol")?;
                // Optional payload argument: `.ok(expr)`.
                let arg = if *self.peek() == TokenKind::LParen {
                    self.advance();
                    let inner = self.parse_expr()?;
                    self.expect(&TokenKind::RParen, "to close an enum payload")?;
                    Some(Box::new(inner))
                } else {
                    None
                };
                Ok(Expr::Symbol { name, arg, span })
            }
            TokenKind::Ident(name) => {
                self.advance();
                // A PascalCase name immediately followed by `{` is a struct
                // literal `Point { x: 1, y: 2 }` (SPEC §11, v0.4). The case
                // convention (§4.1: typenames are PascalCase, values snake_case)
                // disambiguates it from `if cond { ... }` / `while cond { ... }`,
                // whose conditions are lowercase value identifiers.
                if *self.peek() == TokenKind::LBrace
                    && name.chars().next().map_or(false, |c| c.is_ascii_uppercase())
                {
                    return self.parse_struct_lit(name, span);
                }
                if *self.peek() == TokenKind::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    if *self.peek() != TokenKind::RParen {
                        loop {
                            args.push(self.parse_expr()?);
                            if *self.peek() == TokenKind::Comma {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RParen, "to close the argument list")?;
                    Ok(Expr::Call {
                        callee: name,
                        args,
                        span,
                    })
                } else {
                    Ok(Expr::Ident { name, span })
                }
            }
            other => Err(self.err(
                DiagCode::Par010,
                format!("expected an expression, found `{}`", other),
                "expression",
            )),
        }
    }

    /// `Name { field: expr, ... }` — a struct literal (SPEC §11, v0.4). The
    /// name and opening `{` are already consumed-by-lookahead at the call site.
    fn parse_struct_lit(&mut self, name: String, span: Span) -> Result<Expr, Diag> {
        self.expect(&TokenKind::LBrace, "to open a struct literal")?;
        let mut fields = Vec::new();
        if *self.peek() != TokenKind::RBrace {
            loop {
                let fspan = self.span_here();
                let (fname, _) = self.expect_ident("for a struct field name")?;
                self.expect(&TokenKind::Colon, "after the field name")?;
                let value = self.parse_expr()?;
                fields.push(FieldInit { name: fname, value, span: fspan });
                if *self.peek() == TokenKind::Comma {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RBrace, "to close a struct literal")?;
        Ok(Expr::StructLit { name, fields, span })
    }

    fn parse_if(&mut self) -> Result<Expr, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::If, "to start a conditional")?;
        let cond = self.parse_expr()?;
        let then_blk = self.parse_block()?;
        self.expect(&TokenKind::Else, "an `if` expression requires an `else` branch")?;
        let else_blk = self.parse_block()?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then_blk: Box::new(then_blk),
            else_blk: Box::new(else_blk),
            span,
        })
    }

    fn parse_match(&mut self) -> Result<Expr, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::Match, "to start a match")?;
        let scrutinee = self.parse_expr()?;
        self.expect(&TokenKind::LBrace, "to open the match arms")?;
        let mut arms = Vec::new();
        while *self.peek() != TokenKind::RBrace && *self.peek() != TokenKind::Eof {
            let arm_span = self.span_here();
            self.expect(&TokenKind::Dot, "before a match arm symbol")?;
            let (symbol, _) = self.expect_ident("for a match arm symbol")?;
            // Optional payload binder: `.ok(n) =>` or `.ok(_) =>`.
            let binder = if *self.peek() == TokenKind::LParen {
                self.advance();
                let (b, _) = self.expect_ident("for a payload binder")?;
                self.expect(&TokenKind::RParen, "to close a payload binder")?;
                Some(b)
            } else {
                None
            };
            self.expect(&TokenKind::FatArrow, "after a match arm symbol")?;
            let body = self.parse_expr()?;
            arms.push(MatchArm {
                symbol,
                binder,
                body: Box::new(body),
                span: arm_span,
            });
            if *self.peek() == TokenKind::Comma {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "to close the match arms")?;
        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
            span,
        })
    }
}

fn mk_binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    let span = lhs.span();
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        span,
    }
}
