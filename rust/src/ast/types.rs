// fon-parser/src/ast/types.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;

use super::{SchemaId, TypeId};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum TypeExpr {
    Builtin {
        name: String,
        span: Span,
    },
    Named {
        path: String,
        span: Span,
    },
    Generic {
        constructor: String,
        arguments: Vec<TypeId>,
        span: Span,
    },
    Schema {
        schema: SchemaId,
        span: Span,
    },
    Error(super::ErrorNode),
}

impl TypeExpr {
    pub fn kind(&self) -> TypeKind {
        match self {
            Self::Builtin { .. } => TypeKind::Builtin,
            Self::Named { .. } => TypeKind::Named,
            Self::Generic { .. } => TypeKind::Generic,
            Self::Schema { .. } => TypeKind::Schema,
            Self::Error(_) => TypeKind::Error,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Builtin,
    Named,
    Generic,
    Schema,
    Error,
}
