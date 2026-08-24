// fon-parser/src/parser/values.rs

use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::{
    ArrayValue, CstNodeKind, EnumValue, EnumValueKind, ErrorNode, StringPart, StringPartKind,
    StringValue, UnknownShape, UnknownValue, Value,
};
use crate::diagnostic::Diagnostic;
use crate::parser::Parser;
use crate::span::Span;
use crate::token::TokenKind;

impl<'source> Parser<'source> {
    pub(crate) fn parse_value(&mut self) -> crate::ast::ValueId {
        match self.current_kind() {
            TokenKind::True => self.parse_boolean(true),
            TokenKind::False => self.parse_boolean(false),
            TokenKind::Integer | TokenKind::Decimal => self.parse_number(),
            TokenKind::String => self.parse_string(),
            TokenKind::Regex => self.parse_regex(),
            TokenKind::Dot | TokenKind::Identifier => self.parse_enum_or_unknown(),
            TokenKind::UnknownAtom => self.parse_unknown_atom(),
            TokenKind::LBrace => self.parse_object_value(),
            TokenKind::LBracket => self.parse_array_value(),
            TokenKind::Struct | TokenKind::Enum => self.parse_schema_value(),
            _ => self.parse_error_value("expected a value"),
        }
    }

    fn parse_boolean(&mut self, value: bool) -> crate::ast::ValueId {
        let span = self.advance().span;
        self.ast.push_value(Value::Boolean { value, span })
    }

    fn parse_number(&mut self) -> crate::ast::ValueId {
        let token = self.advance();
        let span = token.span;
        self.ast.push_value(Value::Number {
            raw: self.text(span).into(),
            span,
        })
    }

    fn parse_string(&mut self) -> crate::ast::ValueId {
        let token = self.advance();
        let span = token.span;
        let raw = self.text(span).into();
        let parts = self.parse_string_parts(span);
        self.ast
            .push_value(Value::String(StringValue { raw, parts, span }))
    }

    fn parse_string_parts(&self, span: Span) -> Vec<StringPart> {
        let content_start = span.start.saturating_add(1) as usize;
        let content_end = span.end.saturating_sub(1) as usize;
        if content_start >= content_end {
            return Vec::new();
        }

        let content = &self.source[content_start..content_end];
        let mut parts = Vec::new();
        let mut segment_start = 0_usize;
        let mut cursor = 0_usize;
        while cursor < content.len() {
            let next_char = content[cursor..]
                .chars()
                .next()
                .expect("cursor must be valid UTF-8");
            if next_char != '{' {
                cursor += next_char.len_utf8();
                continue;
            }

            if segment_start < cursor {
                parts.push(StringPart {
                    span: Span::new(
                        (content_start + segment_start) as u32,
                        (content_start + cursor) as u32,
                    ),
                    kind: StringPartKind::Text,
                });
            }
            let expression_start = cursor;
            cursor += next_char.len_utf8();
            while cursor < content.len() {
                let expression_char = content[cursor..]
                    .chars()
                    .next()
                    .expect("cursor must be valid UTF-8");
                cursor += expression_char.len_utf8();
                if expression_char == '}' {
                    break;
                }
            }
            parts.push(StringPart {
                span: Span::new(
                    (content_start + expression_start) as u32,
                    (content_start + cursor) as u32,
                ),
                kind: StringPartKind::Interpolation,
            });
            segment_start = cursor;
        }

        if segment_start < content.len() {
            parts.push(StringPart {
                span: Span::new(
                    (content_start + segment_start) as u32,
                    (content_start + content.len()) as u32,
                ),
                kind: StringPartKind::Text,
            });
        }
        if parts.is_empty() {
            parts.push(StringPart {
                span: Span::new(content_start as u32, content_end as u32),
                kind: StringPartKind::Text,
            });
        }
        parts
    }

    fn parse_regex(&mut self) -> crate::ast::ValueId {
        let token = self.advance();
        let span = token.span;
        let raw = self.text(span);
        let closing_slash = raw[1..].rfind('/').map(|offset| offset + 1);
        let Some(closing_slash) = closing_slash else {
            return self.parse_error_value("invalid regular expression");
        };
        let pattern = raw[1..closing_slash].into();
        let flags = if closing_slash + 1 < raw.len() {
            Some(raw[closing_slash + 1..].into())
        } else {
            None
        };
        self.ast.push_value(Value::Regex(crate::ast::RegexValue {
            pattern,
            flags,
            span,
        }))
    }

    fn parse_enum_or_unknown(&mut self) -> crate::ast::ValueId {
        if self.at(TokenKind::Dot) {
            let start = self.advance().span.start;
            let Some(token) = self.eat(TokenKind::Identifier) else {
                return self.parse_error_value("expected an enum variant after '.'");
            };
            let span = Span::new(start, token.span.end);
            return self.ast.push_value(Value::EnumPath(EnumValue {
                kind: EnumValueKind::Shorthand,
                path: self.text(span).into(),
                span,
            }));
        }

        let start_token = self.advance();
        let start = start_token.span.start;
        let mut path = String::from(self.text(start_token.span));
        let mut qualified = false;
        let mut end = start_token.span.end;
        while self.eat(TokenKind::Dot).is_some() {
            qualified = true;
            let Some(token) = self.eat(TokenKind::Identifier) else {
                self.diagnostics.push(Diagnostic::new(
                    "E0301",
                    "expected an identifier after '.'",
                    self.current_span(),
                ));
                break;
            };
            path.push('.');
            path.push_str(self.text(token.span));
            end = token.span.end;
        }
        let span = Span::new(start, end);
        if qualified {
            self.ast.push_value(Value::EnumPath(EnumValue {
                kind: EnumValueKind::Qualified,
                path,
                span,
            }))
        } else {
            self.push_unknown(path, UnknownShape::BareAtom, span)
        }
    }

    fn parse_unknown_atom(&mut self) -> crate::ast::ValueId {
        let token = self.advance();
        let span = token.span;
        let raw: String = self.text(span).into();
        let shape = classify_unknown(raw.as_str());
        self.push_unknown(raw, shape, span)
    }

    fn push_unknown(
        &mut self,
        raw: String,
        shape: UnknownShape,
        span: Span,
    ) -> crate::ast::ValueId {
        self.ast
            .push_value(Value::Unknown(UnknownValue { raw, shape, span }))
    }

    pub(crate) fn parse_array_items(&mut self) -> Vec<crate::ast::ValueId> {
        let open_span = self.advance().span;
        if !self.enter_nested(open_span) {
            self.recover_to_member_boundary();
            return Vec::new();
        }
        let mut items = Vec::new();
        self.skip_separators();
        while !matches!(self.current_kind(), TokenKind::RBracket | TokenKind::Eof) {
            items.push(self.parse_value());
            self.skip_separators();
        }
        if self.eat(TokenKind::RBracket).is_none() {
            let span = self.current_span();
            self.diagnostics
                .push(Diagnostic::new("E0107", "expected closing bracket", span));
        }
        self.leave_nested();
        items
    }

    fn parse_array_value(&mut self) -> crate::ast::ValueId {
        let start = self.current_span().start;
        let items = self.parse_array_items();
        let end = self.current_span().start;
        let span = Span::new(start, end);
        self.ast
            .push_value(Value::Array(ArrayValue { items, span }))
    }

    fn parse_schema_value(&mut self) -> crate::ast::ValueId {
        let schema_id = self.parse_schema();
        let schema = self
            .ast
            .schema(schema_id)
            .expect("schema must be available");
        let span = schema.span;
        let kind = schema.kind;
        self.ast.push_value(Value::Schema(crate::ast::SchemaValue {
            kind,
            schema: schema_id,
            span,
        }))
    }

    fn parse_error_value(&mut self, message: &str) -> crate::ast::ValueId {
        let span = self.current_span();
        self.diagnostics
            .push(Diagnostic::new("E0106", message, span));
        let value_id = self.ast.push_value(Value::Error(ErrorNode {
            message: message.into(),
            span,
        }));
        self.push_cst(CstNodeKind::Error, span, Vec::new());
        value_id
    }
}

fn classify_unknown(raw: &str) -> UnknownShape {
    if raw.starts_with("./") || raw.starts_with("../") || raw.starts_with('/') {
        return UnknownShape::PathLike;
    }
    if raw.starts_with('#') {
        return UnknownShape::ColorLike;
    }
    if raw.starts_with('@') {
        return UnknownShape::PackageLike;
    }
    if raw.starts_with('^') || raw.matches('.').count() >= 2 {
        return UnknownShape::VersionLike;
    }
    if raw
        .chars()
        .all(|next_char| next_char.is_alphanumeric() || next_char == '-' || next_char == '_')
    {
        return UnknownShape::BareAtom;
    }
    UnknownShape::Other
}
