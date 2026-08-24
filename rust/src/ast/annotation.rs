// fon-parser/src/ast/annotation.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;

use super::{Key, ValueId};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: String,
    pub arguments: Vec<AnnotationArgument>,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct AnnotationArgument {
    pub key: Option<Key>,
    pub value: Option<ValueId>,
    pub span: Span,
}
