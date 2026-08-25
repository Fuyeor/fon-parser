// fon-parser/src/resolve.rs

use alloc::borrow::ToOwned;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::{Ast, Binding, Document, Member, Root, TypeExpr, TypeId, Value, ValueId};
use crate::diagnostic::Diagnostic;
use crate::hir::{TypedDocument, TypedMember, TypedRoot, TypedValue};
use crate::scheme::{SchemeError, SchemeResolver, TypeReference};

/// The result of optional semantic resolution.
#[derive(Debug, Clone)]
pub struct ResolveResult {
    pub document: TypedDocument,
    pub diagnostics: Vec<Diagnostic>,
}

impl ResolveResult {
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Resolve names and unknown atoms through an injected scheme.
pub fn resolve(document: &Document, resolver: &dyn SchemeResolver) -> ResolveResult {
    let mut context = ResolverContext {
        ast: &document.ast,
        resolver,
        diagnostics: Vec::new(),
    };
    let root = match &document.ast.root {
        Root::ImplicitObject { members } | Root::ExplicitObject { members } => {
            TypedRoot::Object(context.resolve_members(members))
        }
        Root::Array { items } => TypedRoot::Array(
            items
                .iter()
                .map(|value_id| context.resolve_value(*value_id, None))
                .collect(),
        ),
    };
    ResolveResult {
        document: TypedDocument { root },
        diagnostics: context.diagnostics,
    }
}

struct ResolverContext<'document, 'resolver> {
    ast: &'document Ast,
    resolver: &'resolver dyn SchemeResolver,
    diagnostics: Vec<Diagnostic>,
}

impl<'document, 'resolver> ResolverContext<'document, 'resolver> {
    fn resolve_members(&mut self, member_ids: &[crate::ast::MemberId]) -> Vec<TypedMember> {
        let mut seen_keys = BTreeSet::new();
        let mut members = Vec::with_capacity(member_ids.len());
        for member_id in member_ids {
            let Some(member) = self.ast.member(*member_id) else {
                continue;
            };
            let Some(key) = member_key(member) else {
                members.push(TypedMember {
                    key: String::new(),
                    value: TypedValue::Error {
                        message: String::from("invalid member"),
                        span: crate::Span::default(),
                    },
                    span: crate::Span::default(),
                });
                continue;
            };
            if !seen_keys.insert(key.to_owned()) {
                let span = member_span(member);
                self.diagnostics
                    .push(Diagnostic::new("E1001", "duplicate key in object", span));
            }
            match member {
                Member::Binding(binding) => members.push(self.resolve_binding(binding)),
                Member::TypeDeclaration(declaration) => members.push(TypedMember {
                    key: declaration.name.raw.clone(),
                    value: TypedValue::Schema {
                        name: declaration.name.raw.clone(),
                        span: declaration.span,
                    },
                    span: declaration.span,
                }),
                Member::Error(error) => members.push(TypedMember {
                    key: key.to_owned(),
                    value: TypedValue::Error {
                        message: error.message.clone(),
                        span: error.span,
                    },
                    span: error.span,
                }),
            }
        }
        members
    }

    fn resolve_binding(&mut self, binding: &Binding) -> TypedMember {
        let expected = binding
            .type_annotation
            .and_then(|type_id| self.resolve_type_reference(type_id));
        TypedMember {
            key: binding.key.raw.clone(),
            value: self.resolve_value(binding.value, expected.as_ref()),
            span: binding.span,
        }
    }

    fn resolve_value(&mut self, value_id: ValueId, expected: Option<&TypeReference>) -> TypedValue {
        let Some(value) = self.ast.value(value_id) else {
            return TypedValue::Error {
                message: String::from("missing value"),
                span: crate::Span::default(),
            };
        };
        match value {
            Value::Boolean { value, span } => TypedValue::Boolean {
                value: *value,
                span: *span,
            },
            Value::Number { raw, span } => TypedValue::Number {
                raw: raw.clone(),
                span: *span,
            },
            Value::String(value) => TypedValue::String {
                raw: value.raw.clone(),
                span: value.span,
            },
            Value::Regex(value) => TypedValue::Regex {
                pattern: value.pattern.clone(),
                flags: value.flags.clone(),
                span: value.span,
            },
            Value::EnumPath(value) => TypedValue::EnumPath {
                path: value.path.clone(),
                span: value.span,
            },
            Value::Array(value) => TypedValue::Array(
                value
                    .items
                    .iter()
                    .map(|item_id| self.resolve_value(*item_id, None))
                    .collect(),
            ),
            Value::Object(value) => TypedValue::Object(self.resolve_members(&value.members)),
            Value::Schema(value) => TypedValue::Schema {
                name: match value.kind {
                    crate::SchemaKind::Struct => String::from("struct"),
                    crate::SchemaKind::Enum => String::from("enum"),
                },
                span: value.span,
            },
            Value::Unknown(value) => match self.resolver.resolve_unknown(expected, value) {
                Ok(atom) => TypedValue::Atom(atom),
                Err(error) => {
                    self.diagnostics.push(scheme_diagnostic(value.span, error));
                    TypedValue::Error {
                        message: String::from("scheme could not resolve unknown value"),
                        span: value.span,
                    }
                }
            },
            Value::Expression(expression) => TypedValue::Error {
                message: format_expression_error(expression),
                span: expression.span(),
            },
            Value::Error(error) => TypedValue::Error {
                message: error.message.clone(),
                span: error.span,
            },
        }
    }

    fn resolve_type_reference(&mut self, type_id: TypeId) -> Option<TypeReference> {
        let type_expr = self.ast.types.get(type_id.0 as usize)?;
        match type_expr {
            TypeExpr::Builtin { name, span } => Some(TypeReference {
                name: name.clone(),
                span: Some(*span),
            }),
            TypeExpr::Named { path, span } => match self.resolver.resolve_type(path) {
                Ok(mut reference) => {
                    reference.span = Some(*span);
                    Some(reference)
                }
                Err(error) => {
                    self.diagnostics.push(scheme_diagnostic(*span, error));
                    None
                }
            },
            TypeExpr::Generic {
                constructor, span, ..
            } => Some(TypeReference {
                name: constructor.clone(),
                span: Some(*span),
            }),
            TypeExpr::Schema { span, .. } => Some(TypeReference {
                name: String::from("schema"),
                span: Some(*span),
            }),
            TypeExpr::Error(error) => {
                self.diagnostics
                    .push(Diagnostic::new("E1003", &error.message, error.span));
                None
            }
        }
    }
}

fn member_key(member: &Member) -> Option<&str> {
    match member {
        Member::Binding(binding) => Some(binding.key.raw.as_str()),
        Member::TypeDeclaration(declaration) => Some(declaration.name.raw.as_str()),
        Member::Error(_) => None,
    }
}

fn member_span(member: &Member) -> crate::Span {
    match member {
        Member::Binding(binding) => binding.span,
        Member::TypeDeclaration(declaration) => declaration.span,
        Member::Error(error) => error.span,
    }
}

// Report expression values until semantic evaluation is provided by a higher compiler layer.
fn format_expression_error(expression: &crate::ExpressionValue) -> String {
    match expression {
        crate::ExpressionValue::Unary { .. } => String::from("unresolved unary expression"),
        crate::ExpressionValue::Comparison { .. } => {
            String::from("unresolved comparison expression")
        }
        crate::ExpressionValue::Group { .. } => String::from("unresolved grouped expression"),
        crate::ExpressionValue::Quantifier { .. } => {
            String::from("unresolved quantifier expression")
        }
    }
}

fn scheme_diagnostic(span: crate::Span, error: SchemeError) -> Diagnostic {
    Diagnostic::new("E1002", error.message, span)
}
