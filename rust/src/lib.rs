// fon-parser/src/lib.rs

#![no_std]

extern crate alloc;

mod diagnostic;
mod lexer;
mod parser;
mod resolve;
mod span;
mod token;

pub mod ast;
pub mod cst;
pub mod format;
pub mod hir;
pub mod scheme;

pub use ast::{
    Annotation, AnnotationArgument, ArrayValue, Ast, AstValue, Binding, ComparisonOperator,
    CstNode, CstNodeKind, Document, EnumValue, EnumValueKind, EnumVariant, ErrorNode,
    ExpressionValue, Key, Member, MemberId, NodeId, ObjectValue, ParseResult, QuantifierKind,
    RegexValue, Root, RootKind, Schema, SchemaId, SchemaKind, SchemaValue, StringPart,
    StringPartKind, StringValue, SyntaxTree, TypeDeclaration, TypeExpr, TypeId, TypeKind,
    UnaryOperator, UnknownShape, UnknownValue, Value, ValueId, ValueKind,
};
pub use diagnostic::Diagnostic;
pub use format::{format_canonical, reprint_lossless};
pub use hir::{TypedDocument, TypedMember, TypedRoot, TypedValue};
pub use resolve::{ResolveResult, resolve};
pub use scheme::{SchemeError, SchemeResolver, TypeReference, TypedAtom};
pub use span::Span;
pub use token::{Token, TokenKind, Trivia, TriviaKind};

/// Parser limits that prevent unbounded input growth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    pub max_depth: u32,
    pub max_tokens: u32,
    pub max_token_length: u32,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            max_depth: 256,
            max_tokens: 1_000_000,
            max_token_length: 1_048_576,
        }
    }
}

/// Parse a UTF-8 FON document using the default safety limits.
pub fn parse(source: &str) -> ParseResult {
    parse_with_options(source, ParseOptions::default())
}

/// Parse a UTF-8 FON document using explicit safety limits.
pub fn parse_with_options(source: &str, options: ParseOptions) -> ParseResult {
    let lex_result =
        lexer::Lexer::new(source, options.max_tokens, options.max_token_length).tokenize();
    parser::Parser::new(source, lex_result, options).parse_document()
}

/// Parse a byte slice without performing any external I/O or evaluation.
pub fn parse_bytes(source: &[u8]) -> ParseResult {
    match core::str::from_utf8(source) {
        Ok(text) => parse(text),
        Err(error) => {
            let span = Span::new(error.valid_up_to() as u32, source.len() as u32);
            let document = parse("").document;
            ParseResult {
                document,
                diagnostics: alloc::vec![Diagnostic::new(
                    "E0003",
                    "source is not valid UTF-8",
                    span
                )],
            }
        }
    }
}
