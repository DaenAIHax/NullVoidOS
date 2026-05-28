/// Recursive-descent parser for the `.null` language (Phase 1 MVP).
///
/// The grammar is intentionally small:
///
///   expr   = attrset | list | literal | ident-or-field-access
///   attrset = '{' ( ident '=' expr ';' )* '}'
///   list    = '[' expr* ']'
///   literal = STRING | INT | BOOL | NULL | PATH
///   ident-or-field-access = IDENT ('.' IDENT)*
///
/// Constructs that are intentionally NOT in Phase 1 are detected and
/// produce a PAR001 error with a "not in Phase 1 MVP" message.
use crate::ast::{Attr, Expr, Span};
use crate::diagnostics::{Diag, DiagLevel, DiagCode};
use crate::lexer::{Token, TokenKind, line_col};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    src: &'a str,
    file: String,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, src: &'a str, file: impl Into<String>) -> Self {
        Parser {
            tokens,
            pos: 0,
            src,
            file: file.into(),
        }
    }

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_token(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn span_here(&self) -> Span {
        Span {
            offset: self.tokens[self.pos].offset,
        }
    }

    fn diag(&self, offset: usize, code: DiagCode, message: String, fix: Option<String>) -> Diag {
        let (line, col) = line_col(self.src, offset);
        Diag {
            level: DiagLevel::Error,
            code,
            file: self.file.clone(),
            line,
            col,
            message,
            fix,
        }
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<&Token, Diag> {
        if self.peek() == expected {
            Ok(self.advance())
        } else {
            let tok = self.peek_token();
            Err(self.diag(
                tok.offset,
                DiagCode::Par001,
                format!("expected `{}`, found `{}`", expected, self.peek()),
                None,
            ))
        }
    }

    /// Parse the entire file: one top-level expression.
    pub fn parse_file(&mut self) -> Result<Expr, Diag> {
        let expr = self.parse_expr()?;
        // After the top-level expr we expect EOF.
        if !matches!(self.peek(), TokenKind::Eof) {
            let tok = self.peek_token();
            return Err(self.diag(
                tok.offset,
                DiagCode::Par001,
                format!("unexpected token `{}` after top-level expression", self.peek()),
                None,
            ));
        }
        Ok(expr)
    }

    fn parse_expr(&mut self) -> Result<Expr, Diag> {
        match self.peek() {
            TokenKind::LBrace => self.parse_attrset(),
            TokenKind::LBrack => self.parse_list(),
            TokenKind::String(_) => {
                let tok = self.advance();
                let offset = tok.offset;
                if let TokenKind::String(s) = &tok.kind {
                    Ok(Expr::Str {
                        value: s.clone(),
                        span: Span { offset },
                    })
                } else {
                    unreachable!()
                }
            }
            TokenKind::Int(_) => {
                let tok = self.advance();
                let offset = tok.offset;
                if let TokenKind::Int(n) = tok.kind {
                    Ok(Expr::Int {
                        value: n,
                        span: Span { offset },
                    })
                } else {
                    unreachable!()
                }
            }
            TokenKind::Bool(_) => {
                let tok = self.advance();
                let offset = tok.offset;
                if let TokenKind::Bool(b) = tok.kind {
                    Ok(Expr::Bool {
                        value: b,
                        span: Span { offset },
                    })
                } else {
                    unreachable!()
                }
            }
            TokenKind::Null => {
                let offset = self.span_here().offset;
                self.advance();
                Ok(Expr::Null { span: Span { offset } })
            }
            TokenKind::Ident(_) => self.parse_ident_or_field_access(),
            // Detect deferred constructs and produce helpful errors
            TokenKind::Eof => {
                let offset = self.span_here().offset;
                Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "unexpected end of file; expected an expression".to_string(),
                    None,
                ))
            }
            other => {
                let offset = self.peek_token().offset;
                let msg = match other {
                    TokenKind::Eq => "bare `=` is not a valid expression; did you mean to write `{ key = value; }`?".to_string(),
                    TokenKind::Semi => "unexpected `;`".to_string(),
                    _ => format!("unexpected token `{}`; this construct is not in Phase 1 MVP", other),
                };
                Err(self.diag(offset, DiagCode::Par001, msg, None))
            }
        }
    }

    fn parse_attrset(&mut self) -> Result<Expr, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::LBrace)?;
        let mut attrs = Vec::new();
        loop {
            if matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            // Check for deferred features: `let`, `if`, function pattern
            if let TokenKind::Ident(name) = self.peek() {
                let name = name.clone();
                match name.as_str() {
                    "let" => {
                        let offset = self.peek_token().offset;
                        return Err(self.diag(
                            offset,
                            DiagCode::Par001,
                            "`let in` is not in Phase 1 MVP".to_string(),
                            Some("use a flat attrset instead".to_string()),
                        ));
                    }
                    "if" => {
                        let offset = self.peek_token().offset;
                        return Err(self.diag(
                            offset,
                            DiagCode::Par001,
                            "`if then else` is not in Phase 1 MVP".to_string(),
                            None,
                        ));
                    }
                    "import" => {
                        let offset = self.peek_token().offset;
                        return Err(self.diag(
                            offset,
                            DiagCode::Par001,
                            "imports between `.null` files are not in Phase 1 MVP".to_string(),
                            None,
                        ));
                    }
                    _ => {}
                }
            }
            // Check for function-pattern attrset: `{ arg, ... }:` or `{ arg ? default }:`
            // We detect the `{` is followed by ident then `,` or `?` before the `:`.
            // Simple heuristic: if the current context looks like it might be a function arg,
            // it will fail at the `=` expect below anyway with a clear message.

            // Parse `key = value ;`
            let key_offset = self.peek_token().offset;
            let key = match self.peek() {
                TokenKind::Ident(k) => {
                    let k = k.clone();
                    self.advance();
                    k
                }
                _ => {
                    let offset = self.peek_token().offset;
                    return Err(self.diag(
                        offset,
                        DiagCode::Par001,
                        format!("expected an attribute name (identifier), found `{}`; functions and `let in` are not in Phase 1 MVP", self.peek()),
                        None,
                    ));
                }
            };

            // Detect function pattern: `name: ...` or `name, ...`
            if matches!(self.peek(), TokenKind::Semi) {
                // lone ident followed by `;` — treat as missing `= value`
                let offset = self.peek_token().offset;
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    format!("expected `=` after attribute name `{}`, found `;`", key),
                    Some(format!("{} = <value>;", key)),
                ));
            }

            self.expect(&TokenKind::Eq)?;
            let value = self.parse_expr()?;
            self.expect(&TokenKind::Semi)?;

            attrs.push(Attr {
                key,
                key_span: Span { offset: key_offset },
                value,
            });
        }
        self.expect(&TokenKind::RBrace)?;
        Ok(Expr::AttrSet { attrs, span })
    }

    fn parse_list(&mut self) -> Result<Expr, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::LBrack)?;
        let mut items = Vec::new();
        loop {
            if matches!(self.peek(), TokenKind::RBrack | TokenKind::Eof) {
                break;
            }
            items.push(self.parse_expr()?);
        }
        self.expect(&TokenKind::RBrack)?;
        Ok(Expr::List { items, span })
    }

    fn parse_ident_or_field_access(&mut self) -> Result<Expr, Diag> {
        let offset = self.peek_token().offset;
        let name = match self.peek() {
            TokenKind::Ident(n) => n.clone(),
            _ => unreachable!(),
        };
        self.advance();

        // Detect deferred identifier constructs
        match name.as_str() {
            "let" => {
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "`let in` is not in Phase 1 MVP".to_string(),
                    Some("use a flat attrset instead".to_string()),
                ));
            }
            "if" => {
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "`if then else` is not in Phase 1 MVP".to_string(),
                    None,
                ));
            }
            "import" => {
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "imports between `.null` files are not in Phase 1 MVP".to_string(),
                    None,
                ));
            }
            _ => {}
        }

        let mut expr = Expr::Ident {
            name,
            span: Span { offset },
        };

        // Handle chained field access: `ident.field.field`
        while matches!(self.peek(), TokenKind::Dot) {
            let dot_offset = self.peek_token().offset;
            self.advance(); // consume '.'
            match self.peek() {
                TokenKind::Ident(field) => {
                    let field = field.clone();
                    let field_offset = self.peek_token().offset;
                    self.advance();
                    expr = Expr::FieldAccess {
                        lhs: Box::new(expr),
                        field,
                        span: Span {
                            offset: field_offset,
                        },
                    };
                }
                _ => {
                    return Err(self.diag(
                        dot_offset,
                        DiagCode::Par001,
                        format!("expected field name after `.`, found `{}`", self.peek()),
                        None,
                    ));
                }
            }
        }
        Ok(expr)
    }
}
