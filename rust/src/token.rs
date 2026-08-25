// fon-parser/src/token.rs

use crate::span::Span;

/// Every terminal symbol produced by the FON lexer.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Identifier,
    Integer,
    Decimal,
    UnknownAtom,
    String,
    Regex,
    True,
    False,
    Struct,
    Enum,
    AnnotationStart,
    Equals,
    Colon,
    Comma,
    Dot,
    At,
    Caret,
    Hash,
    Plus,
    Minus,
    Slash,
    Bang,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    LessThan,
    GreaterThan,
    Newline,
    LineComment,
    BlockComment,
    Eof,
    Error,
}

/// A source token with its exact byte span.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Trivia that is not part of the semantic syntax tree.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaKind {
    LineComment,
    BlockComment,
    Whitespace,
}
