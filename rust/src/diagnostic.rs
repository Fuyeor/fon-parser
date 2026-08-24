// fon-parser/src/diagnostic.rs

use alloc::string::String;

use crate::span::Span;

/// A source diagnostic produced during lexing or parsing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub(crate) fn new(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            span,
        }
    }
}
