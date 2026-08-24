// fon-parser/src/ast/schema.rs

use alloc::vec::Vec;

use crate::span::Span;

use super::{AnnotationId, Key, TypeId, ValueId};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct Schema {
    pub kind: SchemaKind,
    pub fields: Vec<StructField>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    Struct,
    Enum,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct StructField {
    pub annotations: Vec<AnnotationId>,
    pub key: Key,
    pub type_annotation: Option<TypeId>,
    pub default_value: Option<ValueId>,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub annotations: Vec<AnnotationId>,
    pub name: Key,
    pub payload: Option<TypeId>,
    pub span: Span,
}
