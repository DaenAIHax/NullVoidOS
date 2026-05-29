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
                other => {
                    return Err(self.err(
                        DiagCode::Par010,
                        format!("expected `fn` or `enum` at top level, found `{}`", other),
                        "fn or enum",
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
            });
        }
        let (name, _) = self.expect_ident("for a type")?;
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
                        // Assignment: `<ident> = <value>;`. Only a bare
                        // variable is assignable (no field/index lvalues yet).
                        let span = expr.span();
                        let name = match expr {
                            Expr::Ident { name, .. } => name,
                            _ => {
                                return Err(self.err(
                                    DiagCode::Par010,
                                    "only a variable can be assigned (left of `=`)",
                                    "an identifier",
                                ))
                            }
                        };
                        self.advance(); // `=`
                        let value = self.parse_expr()?;
                        self.expect(&TokenKind::Semi, "to end the assignment")?;
                        stmts.push(Stmt::Assign { name, value, span });
                    } else if *self.peek() == TokenKind::Semi {
                        self.advance();
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
        self.parse_primary()
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
