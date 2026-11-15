// SPDX-License-Identifier: GPL-3.0-or-later
//! LSL lexer / tokenizer.
//!
//! Converts raw LSL source text into a stream of [`SpannedToken`]s.

use std::fmt;

/// Source location: byte offset, line, column (all 0-based internally but
/// displayed 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub offset: usize,
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line + 1, self.col + 1)
    }
}

/// A token with its source location.
#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// All LSL tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // --- Literals ---
    IntegerLit(i32),
    FloatLit(f64),
    StringLit(String),

    // --- Identifier ---
    Ident(String),

    // --- Type keywords ---
    KwInteger,
    KwFloat,
    KwString,
    KwKey,
    KwVector,
    KwRotation,
    KwList,

    // --- Control keywords ---
    KwDefault,
    KwState,
    KwIf,
    KwElse,
    KwFor,
    KwWhile,
    KwDo,
    KwJump,
    KwReturn,
    KwEvent, // not a real keyword but a placeholder

    // --- Operators ---
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    AmpAmp,
    PipePipe,
    Bang,
    Tilde,
    Amp,
    Pipe,
    Caret,
    LtLt,
    GtGt,
    PlusPlus,
    MinusMinus,

    // --- Punctuation ---
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semi,
    Comma,
    Dot,
    At,

    // --- Special ---
    Eof,
}

impl Token {
    pub fn is_type_keyword(&self) -> bool {
        matches!(
            self,
            Token::KwInteger
                | Token::KwFloat
                | Token::KwString
                | Token::KwKey
                | Token::KwVector
                | Token::KwRotation
                | Token::KwList
        )
    }
}

/// Tokenize LSL source into a vector of spanned tokens.
pub fn lex(source: &str) -> Result<Vec<SpannedToken>, LexError> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token()?;
        let is_eof = tok.token == Token::Eof;
        tokens.push(tok);
        if is_eof {
            break;
        }
    }
    Ok(tokens)
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.span, self.message)
    }
}

struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            pos: 0,
            line: 0,
            col: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.source.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == b'\n' {
            self.line += 1;
            self.col = 0;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn span_from(&self, start_offset: usize, start_line: usize, start_col: usize) -> Span {
        Span {
            offset: start_offset,
            line: start_line,
            col: start_col,
            len: self.pos - start_offset,
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n') => {
                    self.advance();
                }
                Some(b'/') => {
                    if self.peek2() == Some(b'/') {
                        // line comment
                        while let Some(ch) = self.advance() {
                            if ch == b'\n' {
                                break;
                            }
                        }
                    } else if self.peek2() == Some(b'*') {
                        // block comment
                        self.advance(); // /
                        self.advance(); // *
                        loop {
                            match self.advance() {
                                Some(b'*') if self.peek() == Some(b'/') => {
                                    self.advance();
                                    break;
                                }
                                None => break,
                                _ => {}
                            }
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    fn next_token(&mut self) -> Result<SpannedToken, LexError> {
        self.skip_whitespace_and_comments();

        let start_offset = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        let Some(ch) = self.advance() else {
            return Ok(SpannedToken {
                token: Token::Eof,
                span: self.span_from(start_offset, start_line, start_col),
            });
        };

        let make_span = |s: &Self| s.span_from(start_offset, start_line, start_col);

        let token = match ch {
            b'{' => Token::LBrace,
            b'}' => Token::RBrace,
            b'(' => Token::LParen,
            b')' => Token::RParen,
            b'[' => Token::LBracket,
            b']' => Token::RBracket,
            b';' => Token::Semi,
            b',' => Token::Comma,
            b'.' => Token::Dot,
            b'@' => Token::At,
            b'~' => Token::Tilde,
            b'^' => Token::Caret,
            b'+' => {
                if self.peek() == Some(b'+') {
                    self.advance();
                    Token::PlusPlus
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    Token::PlusEq
                } else {
                    Token::Plus
                }
            }
            b'-' => {
                if self.peek() == Some(b'-') {
                    self.advance();
                    Token::MinusMinus
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    Token::MinusEq
                } else {
                    Token::Minus
                }
            }
            b'*' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::StarEq
                } else {
                    Token::Star
                }
            }
            b'/' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::SlashEq
                } else {
                    Token::Slash
                }
            }
            b'%' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::PercentEq
                } else {
                    Token::Percent
                }
            }
            b'=' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::EqEq
                } else {
                    Token::Eq
                }
            }
            b'!' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::BangEq
                } else {
                    Token::Bang
                }
            }
            b'<' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::LtEq
                } else if self.peek() == Some(b'<') {
                    self.advance();
                    Token::LtLt
                } else {
                    Token::Lt
                }
            }
            b'>' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::GtEq
                } else if self.peek() == Some(b'>') {
                    self.advance();
                    Token::GtGt
                } else {
                    Token::Gt
                }
            }
            b'&' => {
                if self.peek() == Some(b'&') {
                    self.advance();
                    Token::AmpAmp
                } else {
                    Token::Amp
                }
            }
            b'|' => {
                if self.peek() == Some(b'|') {
                    self.advance();
                    Token::PipePipe
                } else {
                    Token::Pipe
                }
            }
            b'"' => return self.lex_string(start_offset, start_line, start_col),
            b'0' if self.peek() == Some(b'x') || self.peek() == Some(b'X') => {
                self.advance(); // skip x/X
                let mut val: i64 = 0;
                while let Some(d) = self.peek() {
                    if let Some(digit) = hex_digit(d) {
                        val = val.wrapping_mul(16).wrapping_add(digit as i64);
                        self.advance();
                    } else {
                        break;
                    }
                }
                Token::IntegerLit(val as i32)
            }
            b'0'..=b'9' => return self.lex_number(ch, start_offset, start_line, start_col),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let mut ident = String::new();
                ident.push(ch as char);
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric() || c == b'_' {
                        ident.push(c as char);
                        self.advance();
                    } else {
                        break;
                    }
                }
                keyword_or_ident(&ident)
            }
            other => {
                return Err(LexError {
                    message: format!("unexpected character: '{}'", other as char),
                    span: make_span(self),
                });
            }
        };

        Ok(SpannedToken {
            token,
            span: make_span(self),
        })
    }

    fn lex_string(
        &mut self,
        start_offset: usize,
        start_line: usize,
        start_col: usize,
    ) -> Result<SpannedToken, LexError> {
        let mut s = String::new();
        loop {
            match self.advance() {
                Some(b'"') => break,
                Some(b'\\') => match self.advance() {
                    Some(b'n') => s.push('\n'),
                    Some(b't') => s.push('\t'),
                    Some(b'\\') => s.push('\\'),
                    Some(b'"') => s.push('"'),
                    Some(other) => {
                        s.push('\\');
                        s.push(other as char);
                    }
                    None => {
                        return Err(LexError {
                            message: "unterminated string escape".into(),
                            span: self.span_from(start_offset, start_line, start_col),
                        });
                    }
                },
                Some(ch) => s.push(ch as char),
                None => {
                    return Err(LexError {
                        message: "unterminated string literal".into(),
                        span: self.span_from(start_offset, start_line, start_col),
                    });
                }
            }
        }
        Ok(SpannedToken {
            token: Token::StringLit(s),
            span: self.span_from(start_offset, start_line, start_col),
        })
    }

    fn lex_number(
        &mut self,
        first: u8,
        start_offset: usize,
        start_line: usize,
        start_col: usize,
    ) -> Result<SpannedToken, LexError> {
        let mut num_str = String::new();
        num_str.push(first as char);
        let mut is_float = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                num_str.push(c as char);
                self.advance();
            } else if c == b'.' && !is_float {
                // Check it's not a method call like 1.toString — but LSL doesn't have those,
                // and floats can be like `1.0` or `1.`
                is_float = true;
                num_str.push('.');
                self.advance();
            } else if (c == b'e' || c == b'E') && !num_str.contains('e') && !num_str.contains('E')
            {
                is_float = true;
                num_str.push(c as char);
                self.advance();
                if let Some(b'+' | b'-') = self.peek() {
                    num_str.push(self.advance().unwrap() as char);
                }
            } else {
                break;
            }
        }

        let span = self.span_from(start_offset, start_line, start_col);

        if is_float {
            let val: f64 = num_str.parse().map_err(|_| LexError {
                message: format!("invalid float literal: {num_str}"),
                span,
            })?;
            Ok(SpannedToken {
                token: Token::FloatLit(val),
                span,
            })
        } else {
            let val: i64 = num_str.parse().unwrap_or(0);
            Ok(SpannedToken {
                token: Token::IntegerLit(val as i32),
                span,
            })
        }
    }
}

fn hex_digit(ch: u8) -> Option<u8> {
    match ch {
        b'0'..=b'9' => Some(ch - b'0'),
        b'a'..=b'f' => Some(ch - b'a' + 10),
        b'A'..=b'F' => Some(ch - b'A' + 10),
        _ => None,
    }
}

fn keyword_or_ident(s: &str) -> Token {
    match s {
        "integer" => Token::KwInteger,
        "float" => Token::KwFloat,
        "string" => Token::KwString,
        "key" => Token::KwKey,
        "vector" => Token::KwVector,
        "rotation" => Token::KwRotation,
        "list" => Token::KwList,
        "default" => Token::KwDefault,
        "state" => Token::KwState,
        "if" => Token::KwIf,
        "else" => Token::KwElse,
        "for" => Token::KwFor,
        "while" => Token::KwWhile,
        "do" => Token::KwDo,
        "jump" => Token::KwJump,
        "return" => Token::KwReturn,
        _ => Token::Ident(s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_simple_script() {
        let src = r#"default { state_entry() { llSay(0, "Hello"); } }"#;
        let tokens = lex(src).unwrap();
        assert!(matches!(tokens[0].token, Token::KwDefault));
        assert!(matches!(tokens[1].token, Token::LBrace));
        assert!(matches!(tokens[2].token, Token::Ident(ref s) if s == "state_entry"));
    }

    #[test]
    fn lex_hex_literal() {
        let tokens = lex("0xFF").unwrap();
        assert!(matches!(tokens[0].token, Token::IntegerLit(255)));
    }

    #[test]
    fn lex_float_literal() {
        let tokens = lex("3.14").unwrap();
        assert!(matches!(tokens[0].token, Token::FloatLit(v) if (v - 3.14).abs() < 0.001));
    }

    #[test]
    fn lex_string_escapes() {
        let tokens = lex(r#""hello\nworld""#).unwrap();
        assert!(matches!(tokens[0].token, Token::StringLit(ref s) if s == "hello\nworld"));
    }

    #[test]
    fn lex_operators() {
        let tokens = lex("++ -- += -= *= /= %= == != <= >= && || << >> ").unwrap();
        assert!(matches!(tokens[0].token, Token::PlusPlus));
        assert!(matches!(tokens[1].token, Token::MinusMinus));
        assert!(matches!(tokens[2].token, Token::PlusEq));
        assert!(matches!(tokens[3].token, Token::MinusEq));
        assert!(matches!(tokens[4].token, Token::StarEq));
        assert!(matches!(tokens[5].token, Token::SlashEq));
        assert!(matches!(tokens[6].token, Token::PercentEq));
        assert!(matches!(tokens[7].token, Token::EqEq));
        assert!(matches!(tokens[8].token, Token::BangEq));
        assert!(matches!(tokens[9].token, Token::LtEq));
        assert!(matches!(tokens[10].token, Token::GtEq));
        assert!(matches!(tokens[11].token, Token::AmpAmp));
        assert!(matches!(tokens[12].token, Token::PipePipe));
        assert!(matches!(tokens[13].token, Token::LtLt));
        assert!(matches!(tokens[14].token, Token::GtGt));
    }

    #[test]
    fn lex_comments() {
        let tokens = lex("integer x; // comment\n/* block */ float y;").unwrap();
        assert!(matches!(tokens[0].token, Token::KwInteger));
        assert!(matches!(tokens[1].token, Token::Ident(ref s) if s == "x"));
        assert!(matches!(tokens[2].token, Token::Semi));
        assert!(matches!(tokens[3].token, Token::KwFloat));
        assert!(matches!(tokens[4].token, Token::Ident(ref s) if s == "y"));
    }
}
