// fon-parser/src/parser/types.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::{CstNodeKind, EnumVariant, Key, Schema, SchemaKind, StructField, TypeExpr, Value};
use crate::diagnostic::Diagnostic;
use crate::parser::Parser;
use crate::span::Span;
use crate::token::TokenKind;

impl<'source> Parser<'source> {
    pub(crate) fn parse_type(&mut self) -> crate::ast::TypeId {
        let start = self.current_span().start;
        let type_id = match self.current_kind() {
            TokenKind::Struct | TokenKind::Enum => {
                let schema_id = self.parse_schema();
                let span = self
                    .ast
                    .schema(schema_id)
                    .map(|schema| schema.span)
                    .unwrap_or(self.current_span());
                self.ast.push_type(TypeExpr::Schema {
                    schema: schema_id,
                    span,
                })
            }
            TokenKind::Identifier | TokenKind::UnknownAtom => {
                let constructor_token = self.advance();
                let constructor = self.text(constructor_token.span).into();
                if self.eat(TokenKind::LessThan).is_some() {
                    let mut arguments = Vec::new();
                    self.skip_separators();
                    while !matches!(self.current_kind(), TokenKind::GreaterThan | TokenKind::Eof) {
                        arguments.push(self.parse_type());
                        self.skip_separators();
                        if self.eat(TokenKind::Comma).is_none()
                            && !self.at(TokenKind::GreaterThan)
                            && !self.at(TokenKind::Eof)
                        {
                            self.diagnostics.push(Diagnostic::new(
                                "E0401",
                                "expected ',' or '>' in generic type",
                                self.current_span(),
                            ));
                            break;
                        }
                    }
                    let end = self
                        .eat(TokenKind::GreaterThan)
                        .map(|token| token.span.end)
                        .unwrap_or(self.current_span().end);
                    self.ast.push_type(TypeExpr::Generic {
                        constructor,
                        arguments,
                        span: Span::new(start, end),
                    })
                } else if is_builtin_type(constructor.as_str()) {
                    self.ast.push_type(TypeExpr::Builtin {
                        name: constructor,
                        span: constructor_token.span,
                    })
                } else {
                    self.ast.push_type(TypeExpr::Named {
                        path: constructor,
                        span: constructor_token.span,
                    })
                }
            }
            _ => {
                let span = self.current_span();
                self.diagnostics
                    .push(Diagnostic::new("E0402", "expected a type", span));
                self.ast.push_type(TypeExpr::Error(crate::ast::ErrorNode {
                    message: String::from("expected a type"),
                    span,
                }))
            }
        };
        self.push_cst(
            CstNodeKind::Value,
            Span::new(start, self.type_end(type_id)),
            Vec::new(),
        );
        type_id
    }

    pub(crate) fn parse_schema(&mut self) -> crate::ast::SchemaId {
        let start = self.current_span().start;
        let schema_kind = match self.advance().kind {
            TokenKind::Struct => SchemaKind::Struct,
            TokenKind::Enum => SchemaKind::Enum,
            _ => unreachable!("parse_schema requires a schema keyword"),
        };
        let open_span = self.current_span();
        if self.eat(TokenKind::LBrace).is_none() {
            let span = self.current_span();
            self.diagnostics.push(Diagnostic::new(
                "E0403",
                "expected '{' after schema keyword",
                span,
            ));
            while !matches!(self.current_kind(), TokenKind::RBrace | TokenKind::Eof) {
                self.advance();
            }
            let end = self.current_span().end;
            return self.ast.push_schema(Schema {
                kind: schema_kind,
                fields: Vec::new(),
                variants: Vec::new(),
                span: Span::new(start, end),
            });
        }
        let nested = self.enter_nested(open_span);
        let mut fields = Vec::new();
        let mut variants = Vec::new();
        self.skip_separators();

        if nested {
            while !matches!(self.current_kind(), TokenKind::RBrace | TokenKind::Eof) {
                let annotations = self.parse_leading_annotations();
                match schema_kind {
                    SchemaKind::Struct => fields.push(self.parse_struct_field(annotations)),
                    SchemaKind::Enum => variants.push(self.parse_enum_variant(annotations)),
                }
                self.skip_separators();
            }
        }

        let end = self
            .eat(TokenKind::RBrace)
            .map(|token| token.span.end)
            .unwrap_or_else(|| {
                let span = self.current_span();
                self.diagnostics.push(Diagnostic::new(
                    "E0404",
                    "expected closing brace for schema",
                    span,
                ));
                span.end
            });
        if nested {
            self.leave_nested();
        }
        self.ast.push_schema(Schema {
            kind: schema_kind,
            fields,
            variants,
            span: Span::new(start, end),
        })
    }

    fn parse_struct_field(&mut self, annotations: Vec<crate::ast::AnnotationId>) -> StructField {
        let start = self.current_span().start;
        let key = self.parse_key().unwrap_or_else(|| {
            let span = self.current_span();
            self.diagnostics.push(Diagnostic::new(
                "E0405",
                "expected a struct field name",
                span,
            ));
            Key {
                raw: String::new(),
                span,
            }
        });
        let type_annotation = if self.eat(TokenKind::Colon).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let default_value = if self.eat(TokenKind::Equals).is_some() {
            Some(self.parse_value())
        } else {
            None
        };
        let end = default_value
            .map(|value_id| self.value_end(value_id))
            .or_else(|| type_annotation.map(|type_id| self.type_end(type_id)))
            .unwrap_or(key.span.end);
        StructField {
            annotations,
            key,
            type_annotation,
            default_value,
            span: Span::new(start, end),
        }
    }

    fn parse_enum_variant(&mut self, annotations: Vec<crate::ast::AnnotationId>) -> EnumVariant {
        let start = self.current_span().start;
        let name = self.parse_key().unwrap_or_else(|| {
            let span = self.current_span();
            self.diagnostics.push(Diagnostic::new(
                "E0406",
                "expected an enum variant name",
                span,
            ));
            Key {
                raw: String::new(),
                span,
            }
        });
        let payload = if self.eat(TokenKind::Colon).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let end = payload
            .map(|type_id| self.type_end(type_id))
            .unwrap_or(name.span.end);
        EnumVariant {
            annotations,
            name,
            payload,
            span: Span::new(start, end),
        }
    }

    fn type_end(&self, type_id: crate::ast::TypeId) -> u32 {
        match self.ast.types.get(type_id.0 as usize) {
            Some(TypeExpr::Builtin { span, .. })
            | Some(TypeExpr::Named { span, .. })
            | Some(TypeExpr::Generic { span, .. })
            | Some(TypeExpr::Schema { span, .. })
            | Some(TypeExpr::Error(crate::ast::ErrorNode { span, .. })) => span.end,
            None => self.current_span().end,
        }
    }

    pub(crate) fn value_end(&self, value_id: crate::ast::ValueId) -> u32 {
        match self.ast.value(value_id) {
            Some(Value::Boolean { span, .. })
            | Some(Value::Number { span, .. })
            | Some(Value::String(crate::ast::StringValue { span, .. }))
            | Some(Value::Regex(crate::ast::RegexValue { span, .. }))
            | Some(Value::EnumPath(crate::ast::EnumValue { span, .. }))
            | Some(Value::Array(crate::ast::ArrayValue { span, .. }))
            | Some(Value::Object(crate::ast::ObjectValue { span, .. }))
            | Some(Value::Schema(crate::ast::SchemaValue { span, .. }))
            | Some(Value::Unknown(crate::ast::UnknownValue { span, .. }))
            | Some(Value::Error(crate::ast::ErrorNode { span, .. })) => span.end,
            None => self.current_span().end,
        }
    }
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "string"
            | "bytes"
            | "char"
            | "byte"
            | "int"
            | "float"
            | "void"
            | "never"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "f32"
            | "f64"
    )
}
