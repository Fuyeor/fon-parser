// fon-parser/src/hir.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;

/// A semantically resolved document independent of Fer compiler state.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct TypedDocument {
    pub root: TypedRoot,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum TypedRoot {
    Object(Vec<TypedMember>),
    Array(Vec<TypedValue>),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct TypedMember {
    pub key: String,
    pub value: TypedValue,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum TypedValue {
    Boolean {
        value: bool,
        span: Span,
    },
    Number {
        raw: String,
        span: Span,
    },
    String {
        raw: String,
        span: Span,
    },
    Regex {
        pattern: String,
        flags: Option<String>,
        span: Span,
    },
    EnumPath {
        path: String,
        span: Span,
    },
    Array(Vec<TypedValue>),
    Object(Vec<TypedMember>),
    Schema {
        name: String,
        span: Span,
    },
    Atom(TypedAtom),
    Error {
        message: String,
        span: Span,
    },
}

pub use crate::scheme::TypedAtom;
