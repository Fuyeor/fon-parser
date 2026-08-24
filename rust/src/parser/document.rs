// fon-parser/src/parser/document.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::{
    Annotation, AnnotationArgument, AnnotationId, Binding, CstNodeKind, ErrorNode, Key, Member,
    MemberId, NodeId, Root, TypeDeclaration, Value,
};
use crate::diagnostic::Diagnostic;
use crate::parser::Parser;
use crate::span::Span;
use crate::token::TokenKind;

impl<'source> Parser<'source> {
    pub(crate) fn parse_document(mut self) -> crate::ast::ParseResult {
        self.skip_separators();
        let mut root_annotations = Vec::new();
        while self.at(TokenKind::AnnotationStart) {
            if let Some(annotation_id) = self.parse_annotation() {
                root_annotations.push(annotation_id);
            }
            self.skip_separators();
        }

        let explicit_object = self.at(TokenKind::LBrace);
        let root = if explicit_object {
            let (members, children, span) = self.parse_object_members();
            self.ast.root = Root::ExplicitObject { members };
            self.push_cst(CstNodeKind::Object, span, children);
            self.ast.root.clone()
        } else if self.at(TokenKind::LBracket) {
            if !root_annotations.is_empty() {
                let span = self.current_span();
                self.diagnostics.push(Diagnostic::new(
                    "E0101",
                    "root annotations require an explicit object root",
                    span,
                ));
            }
            let items = self.parse_array_items();
            self.ast.root = Root::Array { items };
            self.ast.root.clone()
        } else {
            if !root_annotations.is_empty() {
                let span = self.current_span();
                self.diagnostics.push(Diagnostic::new(
                    "E0101",
                    "root annotations require an explicit object root",
                    span,
                ));
            }
            let (members, children, span) = self.parse_object_members();
            self.ast.root = Root::ImplicitObject { members };
            self.push_cst(CstNodeKind::Object, span, children);
            self.ast.root.clone()
        };

        self.ast.root_annotations = root_annotations;
        let _ = root;
        let (document, diagnostics) = self.finish();
        crate::ast::ParseResult {
            document,
            diagnostics,
        }
    }

    pub(crate) fn parse_object_value(&mut self) -> crate::ast::ValueId {
        let start = self.current_span().start;
        let nested = self.enter_nested(self.current_span());
        let (members, children, end) = if nested {
            let result = self.parse_object_members();
            self.leave_nested();
            result
        } else {
            while !matches!(self.current_kind(), TokenKind::RBrace | TokenKind::Eof) {
                self.advance();
            }
            let end = self
                .eat(TokenKind::RBrace)
                .map(|token| token.span)
                .unwrap_or(self.current_span());
            (Vec::new(), Vec::new(), end)
        };
        let span = Span::new(start, end.end);
        let value_id = self
            .ast
            .push_value(Value::Object(crate::ast::ObjectValue { members, span }));
        self.push_cst(CstNodeKind::Object, span, children);
        value_id
    }

    pub(crate) fn parse_object_members(&mut self) -> (Vec<MemberId>, Vec<NodeId>, Span) {
        let start = self.current_span().start;
        let explicit = self.eat(TokenKind::LBrace).is_some();
        let mut members = Vec::new();
        let mut children = Vec::new();
        self.skip_separators();

        while !self.at(TokenKind::Eof) && (!explicit || !self.at(TokenKind::RBrace)) {
            if self.at(TokenKind::RBrace) {
                break;
            }
            let annotations = self.parse_leading_annotations();
            match self.parse_member(annotations) {
                Ok((member_id, cst_id)) => {
                    members.push(member_id);
                    children.push(cst_id);
                }
                Err((error_member, cst_id)) => {
                    members.push(error_member);
                    children.push(cst_id);
                }
            }
            self.skip_separators();
        }

        let end = if explicit {
            self.eat(TokenKind::RBrace)
                .map(|token| token.span)
                .unwrap_or_else(|| {
                    let span = self.current_span();
                    self.diagnostics
                        .push(Diagnostic::new("E0103", "expected closing brace", span));
                    span
                })
        } else if members.is_empty() {
            self.current_span()
        } else {
            self.ast_member_end(members.last().copied())
                .unwrap_or(self.current_span())
        };
        (members, children, Span::new(start, end.end))
    }

    fn parse_member(
        &mut self,
        annotations: Vec<AnnotationId>,
    ) -> Result<(MemberId, NodeId), (MemberId, NodeId)> {
        let key = match self.parse_key() {
            Some(key) => key,
            None => {
                let span = self.current_span();
                self.diagnostics
                    .push(Diagnostic::new("E0102", "expected a key", span));
                self.recover_to_member_boundary();
                let error = Member::Error(ErrorNode {
                    message: String::from("expected a key"),
                    span,
                });
                let member_id = self.ast.push_member(error);
                let cst_id = self.push_cst(CstNodeKind::Error, span, Vec::new());
                return Err((member_id, cst_id));
            }
        };

        if self.eat(TokenKind::Colon).is_some() {
            if matches!(self.current_kind(), TokenKind::Struct | TokenKind::Enum) {
                let schema_id = self.parse_schema();
                let span = Span::new(key.span.start, self.schema_end(schema_id));
                let declaration = Member::TypeDeclaration(TypeDeclaration {
                    annotations,
                    name: key,
                    definition: schema_id,
                    span,
                });
                let member_id = self.ast.push_member(declaration);
                let cst_id = self.push_cst(CstNodeKind::TypeDeclaration, span, Vec::new());
                return Ok((member_id, cst_id));
            }
            let type_annotation = self.parse_type();
            self.expect(
                TokenKind::Equals,
                "E0104",
                "expected '=' after type annotation",
            );
            let value = self.parse_value();
            let span = Span::new(key.span.start, self.value_end(value));
            let binding = Member::Binding(Binding {
                annotations,
                key,
                type_annotation: Some(type_annotation),
                value,
                span,
            });
            let member_id = self.ast.push_member(binding);
            let cst_id = self.push_cst(CstNodeKind::Binding, span, Vec::new());
            return Ok((member_id, cst_id));
        }

        self.expect(TokenKind::Equals, "E0105", "expected '=' after key");
        let value = self.parse_value();
        let span = Span::new(key.span.start, self.value_end(value));
        let member_id = self.ast.push_member(Member::Binding(Binding {
            annotations,
            key,
            type_annotation: None,
            value,
            span,
        }));
        let cst_id = self.push_cst(CstNodeKind::Binding, span, Vec::new());
        Ok((member_id, cst_id))
    }

    pub(crate) fn parse_key(&mut self) -> Option<Key> {
        if !matches!(
            self.current_kind(),
            TokenKind::Identifier | TokenKind::UnknownAtom
        ) {
            return None;
        }
        let token = self.advance();
        Some(Key {
            raw: self.text(token.span).into(),
            span: token.span,
        })
    }

    pub(crate) fn parse_leading_annotations(&mut self) -> Vec<AnnotationId> {
        let mut annotations = Vec::new();
        while self.at(TokenKind::AnnotationStart) {
            if let Some(annotation_id) = self.parse_annotation() {
                annotations.push(annotation_id);
            }
            self.skip_separators();
        }
        annotations
    }

    fn parse_annotation(&mut self) -> Option<AnnotationId> {
        let start = self.advance().span.start;
        let name_token = self.eat(TokenKind::Identifier)?;
        let name = self.text(name_token.span).into();
        let mut arguments = Vec::new();

        if self.eat(TokenKind::Equals).is_some() {
            let argument_start = name_token.span.end;
            let value = self.parse_value();
            let argument_end = self.value_end(value);
            arguments.push(AnnotationArgument {
                key: None,
                value: Some(value),
                span: Span::new(argument_start, argument_end),
            });
        }

        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            self.skip_separators();
            if self.at(TokenKind::RBracket) || self.at(TokenKind::Eof) {
                break;
            }
            let argument_start = self.current_span().start;
            let key = if matches!(
                self.current_kind(),
                TokenKind::Identifier | TokenKind::UnknownAtom
            ) && self
                .tokens
                .get(self.position + 1)
                .is_some_and(|token| token.kind == TokenKind::Equals)
            {
                self.parse_key()
            } else {
                None
            };
            let value = if key.is_some() {
                self.expect(
                    TokenKind::Equals,
                    "E0110",
                    "expected '=' in annotation argument",
                );
                Some(self.parse_value())
            } else {
                self.diagnostics.push(Diagnostic::new(
                    "E0111",
                    "expected an annotation argument",
                    self.current_span(),
                ));
                Some(self.parse_value())
            };
            let argument_end = self.value_end(value.expect("annotation value must exist"));
            arguments.push(AnnotationArgument {
                key,
                value,
                span: Span::new(argument_start, argument_end),
            });
            self.skip_separators();
        }

        let end = self
            .eat(TokenKind::RBracket)
            .map(|token| token.span.end)
            .unwrap_or_else(|| {
                let span = self.current_span();
                self.diagnostics
                    .push(Diagnostic::new("E0112", "expected closing ']'", span));
                span.end
            });
        let span = Span::new(start, end);
        let annotation_id = self.ast.push_annotation(Annotation {
            name,
            arguments,
            span,
        });
        self.push_cst(CstNodeKind::Annotation, span, Vec::new());
        Some(annotation_id)
    }

    fn ast_member_end(&self, member_id: Option<MemberId>) -> Option<Span> {
        member_id.and_then(|id| match self.ast.member(id)? {
            Member::Binding(binding) => Some(binding.span),
            Member::TypeDeclaration(declaration) => Some(declaration.span),
            Member::Error(error) => Some(error.span),
        })
    }

    fn schema_end(&self, schema_id: crate::ast::SchemaId) -> u32 {
        self.ast
            .schema(schema_id)
            .map(|schema| schema.span.end)
            .unwrap_or(self.current_span().end)
    }
}
