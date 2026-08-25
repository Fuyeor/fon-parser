// fon-parser/src/lexer.rs

use alloc::vec::Vec;

use crate::diagnostic::Diagnostic;
use crate::span::Span;
use crate::token::{Token, TokenKind, Trivia, TriviaKind};

/// Lexes an in-memory FON source into source-backed tokens.
pub struct Lexer<'source> {
    source: &'source str,
    position: usize,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    diagnostics: Vec<Diagnostic>,
    max_tokens: u32,
    max_token_length: u32,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str, max_tokens: u32, max_token_length: u32) -> Self {
        Self {
            source,
            position: 0,
            tokens: Vec::new(),
            trivia: Vec::new(),
            diagnostics: Vec::new(),
            max_tokens,
            max_token_length,
        }
    }

    pub fn tokenize(mut self) -> LexResult {
        while self.position < self.source.len() {
            self.lex_next();
            if self.tokens.len() as u32 >= self.max_tokens {
                self.push_diagnostic(
                    "E0002",
                    "maximum token count exceeded",
                    self.position,
                    self.position,
                );
                break;
            }
        }

        let eof = self.position as u32;
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(eof, eof),
        });

        LexResult {
            tokens: self.tokens,
            trivia: self.trivia,
            diagnostics: self.diagnostics,
        }
    }

    fn lex_next(&mut self) {
        let Some(next_char) = self.peek_char() else {
            return;
        };

        match next_char {
            ' ' | '\t' | '\r' => self.lex_whitespace(),
            '\n' => self.push_simple(TokenKind::Newline, 1),
            '/' if self.peek_byte(1) == Some(b'/') => self.lex_line_comment(),
            '/' if self.peek_byte(1) == Some(b'*') => self.lex_block_comment(),
            '`' => self.lex_string(),
            '/' => self.lex_regex(),
            '@' | '^' => self.lex_unknown_atom(),
            '#' if self.peek_byte(1) == Some(b'[') => {
                self.push_simple(TokenKind::AnnotationStart, 2)
            }
            '#' => self.lex_unknown_atom(),
            '.' if self.peek_byte(1) == Some(b'/') => self.lex_unknown_atom(),
            '.' => self.push_simple(TokenKind::Dot, 1),
            '{' => self.push_simple(TokenKind::LBrace, 1),
            '}' => self.push_simple(TokenKind::RBrace, 1),
            '[' => self.push_simple(TokenKind::LBracket, 1),
            ']' => self.push_simple(TokenKind::RBracket, 1),
            '(' => self.push_simple(TokenKind::LParen, 1),
            ')' => self.push_simple(TokenKind::RParen, 1),
            ',' => self.push_simple(TokenKind::Comma, 1),
            ':' => self.push_simple(TokenKind::Colon, 1),
            '<' => self.push_simple(TokenKind::LessThan, 1),
            '>' => self.push_simple(TokenKind::GreaterThan, 1),
            '=' => self.push_simple(TokenKind::Equals, 1),
            '!' => self.push_simple(TokenKind::Bang, 1),
            '+' if self.peek_byte(1).is_some_and(|byte| byte.is_ascii_digit()) => self.lex_number(),
            '+' => self.push_simple(TokenKind::Plus, 1),
            '-' if self.peek_byte(1).is_some_and(|byte| byte.is_ascii_digit()) => self.lex_number(),
            '-' => self.push_simple(TokenKind::Minus, 1),
            _ if next_char.is_ascii_digit() => self.lex_number(),
            _ if is_identifier_start(next_char) => self.lex_identifier(),
            _ => self.lex_unknown_atom(),
        }
    }

    fn lex_whitespace(&mut self) {
        let start = self.position;
        while let Some(next_char) = self.peek_char() {
            if !matches!(next_char, ' ' | '\t' | '\r') {
                break;
            }
            self.advance_char();
        }
        self.trivia.push(Trivia {
            kind: TriviaKind::Whitespace,
            span: Span::new(start as u32, self.position as u32),
        });
    }

    fn lex_line_comment(&mut self) {
        let start = self.position;
        self.advance_char();
        self.advance_char();
        while let Some(next_char) = self.peek_char() {
            if next_char == '\n' {
                break;
            }
            self.advance_char();
        }
        self.trivia.push(Trivia {
            kind: TriviaKind::LineComment,
            span: Span::new(start as u32, self.position as u32),
        });
    }

    fn lex_block_comment(&mut self) {
        let start = self.position;
        self.advance_char();
        self.advance_char();
        while self.position < self.source.len() {
            if self.peek_byte(0) == Some(b'*') && self.peek_byte(1) == Some(b'/') {
                self.advance_char();
                self.advance_char();
                self.trivia.push(Trivia {
                    kind: TriviaKind::BlockComment,
                    span: Span::new(start as u32, self.position as u32),
                });
                return;
            }
            self.advance_char();
        }
        self.trivia.push(Trivia {
            kind: TriviaKind::BlockComment,
            span: Span::new(start as u32, self.position as u32),
        });
        self.push_diagnostic("E0201", "unterminated block comment", start, self.position);
    }

    fn lex_string(&mut self) {
        let start = self.position;
        self.advance_char();
        let mut escaped = false;
        while let Some(next_char) = self.peek_char() {
            self.advance_char();
            if escaped {
                escaped = false;
                continue;
            }
            if next_char == '\\' {
                escaped = true;
            } else if next_char == '`' {
                self.push_token(TokenKind::String, start, self.position);
                return;
            }
        }
        self.push_token(TokenKind::Error, start, self.position);
        self.push_diagnostic(
            "E0202",
            "unterminated backtick string",
            start,
            self.position,
        );
    }

    fn lex_regex(&mut self) {
        let start = self.position;
        self.advance_char();
        let mut escaped = false;
        while let Some(next_char) = self.peek_char() {
            self.advance_char();
            if escaped {
                escaped = false;
                continue;
            }
            if next_char == '\\' {
                escaped = true;
                continue;
            }
            if next_char == '/' {
                while self.peek_char().is_some_and(is_identifier_continue) {
                    self.advance_char();
                }
                self.push_token(TokenKind::Regex, start, self.position);
                return;
            }
            if next_char == '\n' {
                break;
            }
        }
        self.push_token(TokenKind::Error, start, self.position);
        self.push_diagnostic(
            "E0203",
            "unterminated regular expression",
            start,
            self.position,
        );
    }

    fn lex_number(&mut self) {
        let start = self.position;
        let mut dot_count = 0_u32;
        let mut valid_decimal = true;
        let mut first_character = true;
        while let Some(next_char) = self.peek_char() {
            if first_character && matches!(next_char, '-' | '+') {
                self.advance_char();
                first_character = false;
                continue;
            }
            first_character = false;
            if next_char == '.' {
                dot_count += 1;
                self.advance_char();
                continue;
            }
            if next_char.is_ascii_alphanumeric() || matches!(next_char, '-' | '+' | '^' | '#') {
                if !next_char.is_ascii_digit() {
                    valid_decimal = false;
                }
                self.advance_char();
                continue;
            }
            break;
        }
        let kind = if valid_decimal && dot_count == 0 {
            TokenKind::Integer
        } else if valid_decimal && dot_count == 1 {
            TokenKind::Decimal
        } else {
            TokenKind::UnknownAtom
        };
        self.push_token(kind, start, self.position);
    }

    fn lex_identifier(&mut self) {
        let start = self.position;
        while self.peek_char().is_some_and(is_identifier_continue) {
            self.advance_char();
        }
        let word = &self.source[start..self.position];
        let kind = match word {
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            _ => TokenKind::Identifier,
        };
        self.push_token(kind, start, self.position);
    }

    fn lex_unknown_atom(&mut self) {
        let start = self.position;
        while let Some(next_char) = self.peek_char() {
            if is_atom_boundary(next_char) {
                break;
            }
            if next_char == '/' && self.peek_byte(1) == Some(b'/') {
                break;
            }
            self.advance_char();
        }
        if self.position == start {
            self.advance_char();
        }
        self.push_token(TokenKind::UnknownAtom, start, self.position);
    }

    fn push_simple(&mut self, kind: TokenKind, byte_length: usize) {
        let start = self.position;
        self.position += byte_length;
        self.push_token(kind, start, self.position);
    }

    fn push_token(&mut self, kind: TokenKind, start: usize, end: usize) {
        let span = Span::new(start as u32, end as u32);
        self.tokens.push(Token { kind, span });
        if end.saturating_sub(start) > self.max_token_length as usize {
            self.push_diagnostic("E0004", "maximum token length exceeded", start, end);
        }
    }

    fn push_diagnostic(&mut self, code: &'static str, message: &str, start: usize, end: usize) {
        self.diagnostics.push(Diagnostic::new(
            code,
            message,
            Span::new(start as u32, end as u32),
        ));
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn peek_byte(&self, offset: usize) -> Option<u8> {
        self.source.as_bytes().get(self.position + offset).copied()
    }

    fn advance_char(&mut self) {
        if let Some(next_char) = self.peek_char() {
            self.position += next_char.len_utf8();
        }
    }
}

pub struct LexResult {
    pub tokens: Vec<Token>,
    pub trivia: Vec<Trivia>,
    pub diagnostics: Vec<Diagnostic>,
}

fn is_identifier_start(next_char: char) -> bool {
    next_char.is_alphabetic() || next_char == '_'
}

fn is_identifier_continue(next_char: char) -> bool {
    next_char.is_alphanumeric() || matches!(next_char, '_' | '-')
}

fn is_atom_boundary(next_char: char) -> bool {
    next_char.is_whitespace()
        || matches!(
            next_char,
            ',' | '}' | ']' | '{' | '[' | '(' | ')' | ':' | '=' | '<' | '>' | '!' | '`'
        )
}
