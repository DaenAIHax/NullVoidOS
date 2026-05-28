/// Hand-rolled lexer for the `.null` language (Phase 1 MVP).
///
/// Token positions are tracked as byte offsets into the source string.
/// Line/col are computed on demand from the source + offset.
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    String(String),
    Int(i64),
    Bool(bool),
    Null,

    // Delimiters
    LBrace,  // {
    RBrace,  // }
    LBrack,  // [
    RBrack,  // ]
    Eq,      // =
    Semi,    // ;
    Dot,     // .

    // Identifiers / keywords
    Ident(String),

    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::String(s) => write!(f, "\"{}\"", s),
            TokenKind::Int(n) => write!(f, "{}", n),
            TokenKind::Bool(b) => write!(f, "{}", b),
            TokenKind::Null => write!(f, "null"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::LBrack => write!(f, "["),
            TokenKind::RBrack => write!(f, "]"),
            TokenKind::Eq => write!(f, "="),
            TokenKind::Semi => write!(f, ";"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::Ident(s) => write!(f, "{}", s),
            TokenKind::Eof => write!(f, "<EOF>"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    /// Byte offset of the first character of this token in the source.
    pub offset: usize,
}

pub struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { src, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.peek().map_or(false, |c| c.is_ascii_whitespace()) {
                self.advance();
            }
            // Skip comment lines
            if self.peek() == Some('#') {
                while self.peek().map_or(false, |c| c != '\n') {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn lex_string(&mut self) -> Result<Token, LexError> {
        let start = self.pos - 1; // already consumed the opening '"'
        let mut s = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(LexError {
                        offset: start,
                        message: "unterminated string literal".to_string(),
                    })
                }
                Some('"') => break,
                Some('\\') => {
                    match self.advance() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('r') => s.push('\r'),
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some(c) => {
                            return Err(LexError {
                                offset: self.pos - c.len_utf8(),
                                message: format!("unknown escape sequence \\{}", c),
                            })
                        }
                        None => {
                            return Err(LexError {
                                offset: self.pos,
                                message: "unterminated escape sequence".to_string(),
                            })
                        }
                    }
                }
                Some(c) => s.push(c),
            }
        }
        Ok(Token {
            kind: TokenKind::String(s),
            offset: start,
        })
    }

    fn lex_number(&mut self, first: char, start: usize) -> Result<Token, LexError> {
        let mut raw = String::new();
        if first == '-' {
            raw.push('-');
        } else {
            raw.push(first);
        }
        while self.peek().map_or(false, |c| c.is_ascii_digit()) {
            raw.push(self.advance().unwrap());
        }
        let n: i64 = raw.parse().map_err(|_| LexError {
            offset: start,
            message: format!("integer literal out of i64 range: {}", raw),
        })?;
        Ok(Token {
            kind: TokenKind::Int(n),
            offset: start,
        })
    }

    fn lex_ident_or_keyword(&mut self, first: char, start: usize) -> Token {
        let mut s = String::new();
        s.push(first);
        while self
            .peek()
            .map_or(false, |c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            s.push(self.advance().unwrap());
        }
        let kind = match s.as_str() {
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            "null" => TokenKind::Null,
            _ => TokenKind::Ident(s),
        };
        Token { kind, offset: start }
    }

    /// Tokenize the entire source.  Returns all tokens including a
    /// terminal `Eof`.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let start = self.pos;
            match self.advance() {
                None => {
                    tokens.push(Token {
                        kind: TokenKind::Eof,
                        offset: start,
                    });
                    break;
                }
                Some('{') => tokens.push(Token { kind: TokenKind::LBrace, offset: start }),
                Some('}') => tokens.push(Token { kind: TokenKind::RBrace, offset: start }),
                Some('[') => tokens.push(Token { kind: TokenKind::LBrack, offset: start }),
                Some(']') => tokens.push(Token { kind: TokenKind::RBrack, offset: start }),
                Some('=') => tokens.push(Token { kind: TokenKind::Eq, offset: start }),
                Some(';') => tokens.push(Token { kind: TokenKind::Semi, offset: start }),
                Some('.') => {
                    // Could be the start of a path: `./...`
                    if self.peek() == Some('/') {
                        self.advance(); // consume '/'
                        let mut path = String::from("./");
                        while self
                            .peek()
                            .map_or(false, |c| !c.is_ascii_whitespace() && c != ';' && c != ',' && c != ']' && c != '}')
                        {
                            path.push(self.advance().unwrap());
                        }
                        tokens.push(Token {
                            kind: TokenKind::String(path), // paths are strings at eval level
                            offset: start,
                        });
                    } else {
                        tokens.push(Token { kind: TokenKind::Dot, offset: start });
                    }
                }
                Some('"') => {
                    let tok = self.lex_string()?;
                    tokens.push(tok);
                }
                Some('-') if self.peek().map_or(false, |c| c.is_ascii_digit()) => {
                    let tok = self.lex_number('-', start)?;
                    tokens.push(tok);
                }
                Some(c) if c.is_ascii_digit() => {
                    let tok = self.lex_number(c, start)?;
                    tokens.push(tok);
                }
                Some(c) if c.is_alphabetic() || c == '_' => {
                    let tok = self.lex_ident_or_keyword(c, start);
                    tokens.push(tok);
                }
                Some(c) => {
                    return Err(LexError {
                        offset: start,
                        message: format!("unexpected character '{}'", c),
                    });
                }
            }
        }
        Ok(tokens)
    }
}

#[derive(Debug)]
pub struct LexError {
    pub offset: usize,
    pub message: String,
}

/// Compute 1-based (line, col) from a byte offset in `src`.
pub fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let before = &src[..offset];
    let line = before.chars().filter(|&c| c == '\n').count() + 1;
    let col = match before.rfind('\n') {
        Some(nl) => offset - nl,
        None => offset + 1,
    };
    (line, col)
}
