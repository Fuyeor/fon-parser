// fon-parser/src/ast/value.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;

use super::{MemberId, SchemaId, SchemaKind, ValueId};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum Value {
    Boolean { value: bool, span: Span },
    Number { raw: String, span: Span },
    String(StringValue),
    Regex(RegexValue),
    EnumPath(EnumValue),
    Array(ArrayValue),
    Object(ObjectValue),
    Schema(SchemaValue),
    Unknown(UnknownValue),
    Expression(ExpressionValue),
    Error(super::ErrorNode),
}

pub type AstValue = Value;

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Boolean { .. } => ValueKind::Boolean,
            Self::Number { .. } => ValueKind::Number,
            Self::String(_) => ValueKind::String,
            Self::Regex(_) => ValueKind::Regex,
            Self::EnumPath(_) => ValueKind::EnumPath,
            Self::Array(_) => ValueKind::Array,
            Self::Object(_) => ValueKind::Object,
            Self::Schema(_) => ValueKind::Schema,
            Self::Unknown(_) => ValueKind::Unknown,
            Self::Expression(_) => ValueKind::Expression,
            Self::Error(_) => ValueKind::Error,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Boolean,
    Number,
    String,
    Regex,
    EnumPath,
    Array,
    Object,
    Schema,
    Unknown,
    Expression,
    Error,
}

/// A parsed Fer expression retained inside a value slot.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum ExpressionValue {
    Unary {
        operator: UnaryOperator,
        operand: ValueId,
        span: Span,
    },
    Comparison {
        left: ValueId,
        operator: ComparisonOperator,
        right: ValueId,
        span: Span,
    },
    Group {
        expression: ValueId,
        span: Span,
    },
    Quantifier {
        kind: QuantifierKind,
        conditions: Vec<ValueId>,
        span: Span,
    },
}

impl ExpressionValue {
    pub fn span(&self) -> Span {
        match self {
            Self::Unary { span, .. }
            | Self::Comparison { span, .. }
            | Self::Group { span, .. }
            | Self::Quantifier { span, .. } => *span,
        }
    }
}

/// A unary condition operator defined by Fer expressions.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
}

/// A comparison operator defined by Fer condition expressions.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equals,
    Contains,
    In,
    Matches,
    Starts,
    Ends,
}

/// A logical quantifier that combines condition expressions.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantifierKind {
    All,
    Any,
    One,
    None,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct StringValue {
    pub raw: String,
    pub parts: Vec<StringPart>,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct StringPart {
    pub span: Span,
    pub kind: StringPartKind,
}

impl StringPart {
    pub fn is_text(&self) -> bool {
        matches!(self.kind, StringPartKind::Text)
    }

    pub fn is_interpolation(&self) -> bool {
        matches!(self.kind, StringPartKind::Interpolation)
    }

    pub fn text<'source>(&self, source: &'source str) -> Option<&'source str> {
        if !self.is_text() {
            return None;
        }
        source.get(self.span.start as usize..self.span.end as usize)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringPartKind {
    Text,
    Interpolation,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct RegexValue {
    pub pattern: String,
    pub flags: Option<String>,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct EnumValue {
    pub kind: EnumValueKind,
    pub path: String,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumValueKind {
    Shorthand,
    Qualified,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct ArrayValue {
    pub items: Vec<ValueId>,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct ObjectValue {
    pub members: Vec<MemberId>,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct SchemaValue {
    pub kind: SchemaKind,
    pub schema: SchemaId,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct UnknownValue {
    pub raw: String,
    pub shape: UnknownShape,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownShape {
    BareAtom,
    PackageLike,
    PathLike,
    VersionLike,
    ColorLike,
    Other,
}
