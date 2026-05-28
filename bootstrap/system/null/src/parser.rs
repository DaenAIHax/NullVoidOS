/// Recursive-descent parser for the `.null` language (v2).
///
/// Grammar:
///
///   expr      = attrset | list | literal | ident-or-field-access
///             | symbol | capability
///   attrset   = '{' ( ident '=' expr ';' )* '}'
///   list      = '[' expr* ']'
///   literal   = STRING | INT | BOOL | NULL
///   ident-or-field-access = IDENT ('.' IDENT)*
///   symbol    = '.' IDENT
///   capability = '!' IDENT ('.' IDENT)* ('.' STRING)?
///
/// Anti-features (SPEC §2): functions, `let in`, `if then else`, imports,
/// string interpolation. Each is detected and produces PAR001 with a hint.
use crate::ast::{Attr, Expr, Span};
use crate::diagnostics::{span_at, Diag, DiagCode, DiagLevel, Repair};
use crate::lexer::{line_col, Token, TokenKind};
use serde_json::json;

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

    fn diag(
        &self,
        offset: usize,
        code: DiagCode,
        message: String,
        expected: impl Into<String>,
        actual: impl Into<String>,
        repair: Option<Repair>,
    ) -> Diag {
        let (line, col) = line_col(self.src, offset);
        Diag {
            level: DiagLevel::Error,
            code,
            message,
            expected: expected.into(),
            actual: actual.into(),
            file: self.file.clone(),
            span: span_at(line, col),
            repair,
        }
    }

    fn expect(&mut self, expected: &TokenKind) -> Result<&Token, Diag> {
        if self.peek() == expected {
            Ok(self.advance())
        } else {
            let tok_offset = self.peek_token().offset;
            let actual_str = format!("`{}`", self.peek());
            let expected_str = format!("`{}`", expected);
            Err(self.diag(
                tok_offset,
                DiagCode::Par001,
                format!("expected `{}`, found `{}`", expected, self.peek()),
                expected_str,
                actual_str,
                None,
            ))
        }
    }

    /// Parse the entire file: one top-level expression.
    pub fn parse_file(&mut self) -> Result<Expr, Diag> {
        let expr = self.parse_expr()?;
        if !matches!(self.peek(), TokenKind::Eof) {
            let tok = self.peek_token();
            return Err(self.diag(
                tok.offset,
                DiagCode::Par001,
                format!(
                    "unexpected token `{}` after top-level expression",
                    self.peek()
                ),
                "end of file",
                format!("`{}`", self.peek()),
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
                Ok(Expr::Null {
                    span: Span { offset },
                })
            }
            TokenKind::Ident(_) => self.parse_ident_or_field_access(),
            TokenKind::Dot => self.parse_symbol(),
            TokenKind::Bang => self.parse_capability(),
            TokenKind::Eof => {
                let offset = self.span_here().offset;
                Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "unexpected end of file; expected an expression".to_string(),
                    "expression",
                    "end of file",
                    None,
                ))
            }
            other => {
                let offset = self.peek_token().offset;
                let (msg, expected_str) = match other {
                    TokenKind::Eq => (
                        "bare `=` is not a valid expression; did you mean to write `{ key = value; }`?".to_string(),
                        "expression",
                    ),
                    TokenKind::Semi => ("unexpected `;`".to_string(), "expression"),
                    _ => (
                        format!(
                            "unexpected token `{}`; this construct is not in v2",
                            other
                        ),
                        "expression",
                    ),
                };
                Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    msg,
                    expected_str,
                    format!("`{}`", other),
                    None,
                ))
            }
        }
    }

    /// Parse a symbol literal: `.identifier`.
    /// Caller has already confirmed `peek() == Dot`.
    fn parse_symbol(&mut self) -> Result<Expr, Diag> {
        let dot_offset = self.peek_token().offset;
        self.advance(); // consume '.'
        let name_offset = self.peek_token().offset;
        let name = match self.peek() {
            TokenKind::Ident(n) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => {
                return Err(self.diag(
                    dot_offset,
                    DiagCode::Par001,
                    format!(
                        "expected identifier after `.` (symbol literal), found `{}`",
                        self.peek()
                    ),
                    "identifier after `.`",
                    format!("`{}`", self.peek()),
                    None,
                ));
            }
        };
        Ok(Expr::Symbol {
            name,
            span: Span { offset: name_offset },
        })
    }

    /// Parse a capability literal: `!ident(.ident)*(."str")?`.
    /// Caller has already confirmed `peek() == Bang`.
    fn parse_capability(&mut self) -> Result<Expr, Diag> {
        let bang_offset = self.peek_token().offset;
        self.advance(); // consume '!'

        let mut path = Vec::new();
        match self.peek() {
            TokenKind::Ident(n) => {
                path.push(n.clone());
                self.advance();
            }
            _ => {
                return Err(self.diag(
                    bang_offset,
                    DiagCode::Par001,
                    format!(
                        "expected identifier after `!` (capability literal), found `{}`",
                        self.peek()
                    ),
                    "identifier after `!`",
                    format!("`{}`", self.peek()),
                    None,
                ));
            }
        }

        let mut arg: Option<String> = None;
        while matches!(self.peek(), TokenKind::Dot) {
            let dot_offset = self.peek_token().offset;
            self.advance(); // consume '.'
            match self.peek() {
                TokenKind::Ident(n) => {
                    path.push(n.clone());
                    self.advance();
                }
                TokenKind::String(s) => {
                    arg = Some(s.clone());
                    self.advance();
                    if matches!(self.peek(), TokenKind::Dot) {
                        let extra_offset = self.peek_token().offset;
                        return Err(self.diag(
                            extra_offset,
                            DiagCode::Par001,
                            "capability argument must be the last component"
                                .to_string(),
                            "end of capability literal",
                            "extra segment after string argument",
                            None,
                        ));
                    }
                    break;
                }
                _ => {
                    return Err(self.diag(
                        dot_offset,
                        DiagCode::Par001,
                        format!(
                            "expected identifier or string after `.` in capability, found `{}`",
                            self.peek()
                        ),
                        "identifier or string",
                        format!("`{}`", self.peek()),
                        None,
                    ));
                }
            }
        }

        Ok(Expr::Capability {
            path,
            arg,
            span: Span { offset: bang_offset },
        })
    }

    fn parse_attrset(&mut self) -> Result<Expr, Diag> {
        let span = self.span_here();
        self.expect(&TokenKind::LBrace)?;
        let mut attrs = Vec::new();
        loop {
            if matches!(self.peek(), TokenKind::RBrace | TokenKind::Eof) {
                break;
            }
            // Anti-features (SPEC §2): detect and reject early with a hint.
            if let TokenKind::Ident(name) = self.peek() {
                let name = name.clone();
                let offset = self.peek_token().offset;
                match name.as_str() {
                    "let" => {
                        return Err(self.diag(
                            offset,
                            DiagCode::Par001,
                            "`let in` is not in v2".to_string(),
                            "attrset key or `}`",
                            "`let`",
                            None,
                        ));
                    }
                    "if" => {
                        return Err(self.diag(
                            offset,
                            DiagCode::Par001,
                            "`if then else` is not in v2".to_string(),
                            "attrset key or `}`",
                            "`if`",
                            None,
                        ));
                    }
                    "import" => {
                        return Err(self.diag(
                            offset,
                            DiagCode::Par001,
                            "imports between `.null` files are not in v2.0 (see SPEC §12)"
                                .to_string(),
                            "attrset key or `}`",
                            "`import`",
                            None,
                        ));
                    }
                    _ => {}
                }
            }

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
                        format!(
                            "expected an attribute name (identifier), found `{}`; functions and `let in` are not in v2",
                            self.peek()
                        ),
                        "identifier",
                        format!("`{}`", self.peek()),
                        None,
                    ));
                }
            };

            if matches!(self.peek(), TokenKind::Semi) {
                let offset = self.peek_token().offset;
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    format!(
                        "expected `=` after attribute name `{}`, found `;`",
                        key
                    ),
                    "`=`",
                    "`;`",
                    Some(Repair::new(
                        "add-required-field",
                        json!({ "field": key, "type": "Value" }),
                    )),
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

        match name.as_str() {
            "let" => {
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "`let in` is not in v2".to_string(),
                    "expression",
                    "`let`",
                    None,
                ));
            }
            "if" => {
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "`if then else` is not in v2".to_string(),
                    "expression",
                    "`if`",
                    None,
                ));
            }
            "import" => {
                return Err(self.diag(
                    offset,
                    DiagCode::Par001,
                    "imports between `.null` files are not in v2.0 (see SPEC §12)"
                        .to_string(),
                    "expression",
                    "`import`",
                    None,
                ));
            }
            _ => {}
        }

        let mut expr = Expr::Ident {
            name,
            span: Span { offset },
        };

        while matches!(self.peek(), TokenKind::Dot) {
            let dot_offset = self.peek_token().offset;
            self.advance();
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
                        "identifier after `.`",
                        format!("`{}`", self.peek()),
                        None,
                    ));
                }
            }
        }
        Ok(expr)
    }
}
