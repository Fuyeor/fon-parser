// fon-parser/src/scheme.rs

use alloc::string::String;

use crate::{Span, UnknownValue};

/// Resolves syntax-level names and unknown atoms without owning compiler state.
pub trait SchemeResolver: Send + Sync {
    fn resolve_type(&self, name: &str) -> Result<TypeReference, SchemeError>;

    fn resolve_unknown(
        &self,
        expected_type: Option<&TypeReference>,
        value: &UnknownValue,
    ) -> Result<TypedAtom, SchemeError>;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReference {
    pub name: String,
    pub span: Option<Span>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedAtom {
    pub type_name: String,
    pub raw: String,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemeError {
    pub message: String,
}
