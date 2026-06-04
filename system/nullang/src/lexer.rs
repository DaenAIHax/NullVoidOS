//! Hand-rolled lexer for Nullang's construction core. Byte-offset token
//! positions; (line, col) computed on demand. Mirrors the `.null` lexer's
//! conventions (SPEC §4.1) and extends it with construction tokens.
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Str(String),
    Int(i64),
    Bool(bool),

    // Keywords
    Fn,
    Let,
    Uses,
    Return,
    If,
    Else,
    Enum,
    Match,
    Mut,
    While,
    Type,

    // Identifiers (values and type names; the parser disambiguates by position)
    Ident(String),

    // Delimiters / punctuation
    LParen,   // (
    RParen,   // )
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]
    Comma,    // ,
    Semi,     // ;
    Colon,    // :
    Eq,       // =
    Arrow,    // ->
    Dot,      // .
    Bang,     // !  (capability prefix, SPEC §5.5)

    // Operators (SPEC §4)
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %
    EqEq,     // ==
    BangEq,   // !=
    Lt,       // <
    Le,       // <=
    Gt,       // >
    Ge,       // >=
    AmpAmp,   // &&
    PipePipe, // ||
    Pipe,     // |   (enum variant separator)
    FatArrow, // =>  (match arm)

    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TokenKind::*;
        match self {
            Str(s) => write!(f, "\"{}\"", s),
            Int(n) => write!(f, "{}", n),
            Bool(b) => write!(f, "{}", b),
            Fn => write!(f, "fn"),
            Let => write!(f, "let"),
            Uses => write!(f, "uses"),
            Return => write!(f, "return"),
            If => write!(f, "if"),
            Else => write!(f, "else"),
            Enum => write!(f, "enum"),
            Match => write!(f, "match"),
            Mut => write!(f, "mut"),
            While => write!(f, "while"),
            Type => write!(f, "type"),
            Ident(s) => write!(f, "{}", s),
            LParen => write!(f, "("),
            RParen => write!(f, ")"),
            LBrace => write!(f, "{{"),
            RBrace => write!(f, "}}"),
            LBracket => write!(f, "["),
            RBracket => write!(f, "]"),
            Comma => write!(f, ","),
            Semi => write!(f, ";"),
            Colon => write!(f, ":"),
            Eq => write!(f, "="),
            Arrow => write!(f, "->"),
            Dot => write!(f, "."),
            Bang => write!(f, "!"),
            Plus => write!(f, "+"),
            Minus => write!(f, "-"),
            Star => write!(f, "*"),
            Slash => write!(f, "/"),
            Percent => write!(f, "%"),
            EqEq => write!(f, "=="),
            BangEq => write!(f, "!="),
            Lt => write!(f, "<"),
            Le => write!(f, "<="),
            Gt => write!(f, ">"),
            Ge => write!(f, ">="),
            AmpAmp => write!(f, "&&"),
            PipePipe => write!(f, "||"),
            Pipe => write!(f, "|"),
            FatArrow => write!(f, "=>"),
            Eof => write!(f, "<EOF>"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub offset: usize,
}

pub struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

#[derive(Debug)]
pub struct LexError {
    pub offset: usize,
    pub message: String,
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

    /// Consume `c` if it is next; report whether it was consumed.
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().map_or(false, |c| c.is_ascii_whitespace()) {
                self.advance();
            }
            if self.peek() == Some('#') {
                while self.peek().map_or(false, |c| c != '\n') {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn lex_string(&mut self, start: usize) -> Result<Token, LexError> {
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
                Some('\\') => match self.advance() {
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
                },
                Some(c) => s.push(c),
            }
        }
        Ok(Token {
            kind: TokenKind::Str(s),
            offset: start,
        })
    }

    fn lex_number(&mut self, first: char, start: usize) -> Result<Token, LexError> {
        let mut raw = String::new();
        raw.push(first);
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

    fn lex_word(&mut self, first: char, start: usize) -> Token {
        let mut s = String::new();
        s.push(first);
        while self
            .peek()
            .map_or(false, |c| c.is_alphanumeric() || c == '_')
        {
            s.push(self.advance().unwrap());
        }
        let kind = match s.as_str() {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "uses" => TokenKind::Uses,
            "return" => TokenKind::Return,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "enum" => TokenKind::Enum,
            "match" => TokenKind::Match,
            "mut" => TokenKind::Mut,
            "while" => TokenKind::While,
            "type" => TokenKind::Type,
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            _ => TokenKind::Ident(s),
        };
        Token { kind, offset: start }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia();
            let start = self.pos;
            let c = match self.advance() {
                None => {
                    tokens.push(Token {
                        kind: TokenKind::Eof,
                        offset: start,
                    });
                    break;
                }
                Some(c) => c,
            };
            let kind = match c {
                '(' => TokenKind::LParen,
                ')' => TokenKind::RParen,
                '{' => TokenKind::LBrace,
                '}' => TokenKind::RBrace,
                '[' => TokenKind::LBracket,
                ']' => TokenKind::RBracket,
                ',' => TokenKind::Comma,
                ';' => TokenKind::Semi,
                ':' => TokenKind::Colon,
                '.' => TokenKind::Dot,
                '+' => TokenKind::Plus,
                '*' => TokenKind::Star,
                '/' => TokenKind::Slash,
                '%' => TokenKind::Percent,
                '=' if self.eat('=') => TokenKind::EqEq,
                '=' if self.eat('>') => TokenKind::FatArrow,
                '=' => TokenKind::Eq,
                '!' if self.eat('=') => TokenKind::BangEq,
                '!' => TokenKind::Bang,
                '<' if self.eat('=') => TokenKind::Le,
                '<' => TokenKind::Lt,
                '>' if self.eat('=') => TokenKind::Ge,
                '>' => TokenKind::Gt,
                '&' if self.eat('&') => TokenKind::AmpAmp,
                '|' if self.eat('|') => TokenKind::PipePipe,
                '|' => TokenKind::Pipe,
                '-' if self.eat('>') => TokenKind::Arrow,
                '-' => TokenKind::Minus,
                '"' => {
                    let tok = self.lex_string(start)?;
                    tokens.push(tok);
                    continue;
                }
                c if c.is_ascii_digit() => {
                    let tok = self.lex_number(c, start)?;
                    tokens.push(tok);
                    continue;
                }
                c if c.is_alphabetic() || c == '_' => {
                    tokens.push(self.lex_word(c, start));
                    continue;
                }
                c => {
                    return Err(LexError {
                        offset: start,
                        message: format!("unexpected character '{}'", c),
                    })
                }
            };
            tokens.push(Token { kind, offset: start });
        }
        Ok(tokens)
    }
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
