// fon-parser/src/ast/mod.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::span::Span;
use crate::token::{Token, Trivia};

pub mod annotation;
pub mod schema;
pub mod types;
pub mod value;

pub use annotation::{Annotation, AnnotationArgument};
pub use schema::{EnumVariant, Schema, SchemaKind, StructField};
pub use types::{TypeExpr, TypeKind};
pub use value::{
    ArrayValue, AstValue, EnumValue, EnumValueKind, ObjectValue, RegexValue, SchemaValue,
    StringPart, StringPartKind, StringValue, UnknownShape, UnknownValue, Value, ValueKind,
};

macro_rules! define_id {
    ($name:ident) => {
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub u32);
    };
}

define_id!(NodeId);
define_id!(MemberId);
define_id!(ValueId);
define_id!(TypeId);
define_id!(SchemaId);
define_id!(AnnotationId);

/// The parsed document and its source-backed syntax projections.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct Document {
    pub(crate) source: String,
    pub cst: SyntaxTree,
    pub ast: Ast,
}

impl Document {
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// The result of a syntactic parse.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub document: Document,
    pub diagnostics: Vec<crate::diagnostic::Diagnostic>,
}

impl ParseResult {
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// A lossless indexed syntax tree.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SyntaxTree {
    pub nodes: Vec<CstNode>,
    pub tokens: Vec<Token>,
    pub trivia: Vec<Trivia>,
}

impl SyntaxTree {
    pub fn has_error_nodes(&self) -> bool {
        self.nodes
            .iter()
            .any(|node| node.kind == CstNodeKind::Error)
    }

    pub(crate) fn push(&mut self, kind: CstNodeKind, span: Span, children: Vec<NodeId>) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(CstNode {
            id,
            kind,
            span,
            children,
        });
        id
    }
}

/// A concrete node retained for source mapping and recovery.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct CstNode {
    pub id: NodeId,
    pub kind: CstNodeKind,
    pub span: Span,
    pub children: Vec<NodeId>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CstNodeKind {
    Document,
    Object,
    Array,
    Binding,
    TypeDeclaration,
    Value,
    Annotation,
    Error,
}

/// The syntax AST with source-backed fields and indexed child references.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct Ast {
    pub(crate) root: Root,
    pub(crate) root_annotations: Vec<AnnotationId>,
    pub(crate) members: Vec<Member>,
    pub(crate) values: Vec<Value>,
    pub(crate) types: Vec<TypeExpr>,
    pub(crate) schemas: Vec<Schema>,
    pub(crate) annotations: Vec<Annotation>,
}

impl Ast {
    pub(crate) fn new(root: Root) -> Self {
        Self {
            root,
            root_annotations: Vec::new(),
            members: Vec::new(),
            values: Vec::new(),
            types: Vec::new(),
            schemas: Vec::new(),
            annotations: Vec::new(),
        }
    }

    pub fn root_kind(&self) -> RootKind {
        self.root.kind()
    }

    pub fn root_annotations(&self) -> &[AnnotationId] {
        &self.root_annotations
    }

    pub fn object_members(&self) -> Option<&[MemberId]> {
        match &self.root {
            Root::ImplicitObject { members } | Root::ExplicitObject { members } => Some(members),
            Root::Array { .. } => None,
        }
    }

    pub fn root_array_items(&self) -> Option<&[ValueId]> {
        match &self.root {
            Root::Array { items } => Some(items),
            Root::ImplicitObject { .. } | Root::ExplicitObject { .. } => None,
        }
    }

    pub fn member(&self, id: MemberId) -> Option<&Member> {
        self.members.get(id.0 as usize)
    }

    pub fn value(&self, id: ValueId) -> Option<&Value> {
        self.values.get(id.0 as usize)
    }

    pub fn value_kind(&self, id: ValueId) -> ValueKind {
        self.value(id).map(Value::kind).unwrap_or(ValueKind::Error)
    }

    pub fn type_kind(&self, id: TypeId) -> TypeKind {
        self.types
            .get(id.0 as usize)
            .map(TypeExpr::kind)
            .unwrap_or(TypeKind::Error)
    }

    pub fn schema(&self, id: SchemaId) -> Option<&Schema> {
        self.schemas.get(id.0 as usize)
    }

    pub fn annotation(&self, id: AnnotationId) -> Option<&Annotation> {
        self.annotations.get(id.0 as usize)
    }

    pub fn member_key_text(&self, id: MemberId) -> Option<&str> {
        self.member(id).and_then(Member::key_text)
    }

    pub(crate) fn push_member(&mut self, member: Member) -> MemberId {
        let id = MemberId(self.members.len() as u32);
        self.members.push(member);
        id
    }

    pub(crate) fn push_value(&mut self, value: Value) -> ValueId {
        let id = ValueId(self.values.len() as u32);
        self.values.push(value);
        id
    }

    pub(crate) fn push_type(&mut self, type_expr: TypeExpr) -> TypeId {
        let id = TypeId(self.types.len() as u32);
        self.types.push(type_expr);
        id
    }

    pub(crate) fn push_schema(&mut self, schema: Schema) -> SchemaId {
        let id = SchemaId(self.schemas.len() as u32);
        self.schemas.push(schema);
        id
    }

    pub(crate) fn push_annotation(&mut self, annotation: Annotation) -> AnnotationId {
        let id = AnnotationId(self.annotations.len() as u32);
        self.annotations.push(annotation);
        id
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum Root {
    ImplicitObject { members: Vec<MemberId> },
    ExplicitObject { members: Vec<MemberId> },
    Array { items: Vec<ValueId> },
}

impl Root {
    fn kind(&self) -> RootKind {
        match self {
            Self::ImplicitObject { .. } => RootKind::ImplicitObject,
            Self::ExplicitObject { .. } => RootKind::ExplicitObject,
            Self::Array { .. } => RootKind::Array,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    ImplicitObject,
    ExplicitObject,
    Array,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum Member {
    Binding(Binding),
    TypeDeclaration(TypeDeclaration),
    Error(ErrorNode),
}

impl Member {
    pub fn binding(&self) -> Option<&Binding> {
        match self {
            Self::Binding(binding) => Some(binding),
            Self::TypeDeclaration(_) | Self::Error(_) => None,
        }
    }

    pub fn type_declaration(&self) -> Option<&TypeDeclaration> {
        match self {
            Self::TypeDeclaration(declaration) => Some(declaration),
            Self::Binding(_) | Self::Error(_) => None,
        }
    }

    fn key_text(&self) -> Option<&str> {
        match self {
            Self::Binding(binding) => Some(binding.key.raw.as_str()),
            Self::TypeDeclaration(declaration) => Some(declaration.name.raw.as_str()),
            Self::Error(_) => None,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct Binding {
    pub annotations: Vec<AnnotationId>,
    pub key: Key,
    pub type_annotation: Option<TypeId>,
    pub value: ValueId,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct TypeDeclaration {
    pub annotations: Vec<AnnotationId>,
    pub name: Key,
    pub definition: SchemaId,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct Key {
    pub raw: String,
    pub span: Span,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct ErrorNode {
    pub message: String,
    pub span: Span,
}
