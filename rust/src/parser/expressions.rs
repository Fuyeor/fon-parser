// fon-parser/src/parser/expressions.rs

use alloc::vec::Vec;

use crate::ast::{
    ComparisonOperator, CstNodeKind, ExpressionValue, QuantifierKind, Value, ValueId,
};
use crate::diagnostic::Diagnostic;
use crate::parser::Parser;
use crate::span::Span;
use crate::token::{Token, TokenKind};

impl<'source> Parser<'source> {
    /// Parse a value slot as either a regular FON value or a condition expression.
    pub(crate) fn parse_condition_expression(&mut self) -> ValueId {
        if let Some(kind) = self.current_quantifier() {
            return self.parse_quantifier(kind);
        }
        if self.is_unary_not_start() {
            return self.parse_unary();
        }
        if self.at(TokenKind::LParen) {
            return self.parse_group();
        }

        let left = self.parse_value_atom();
        let Some(operator) = self.parse_comparison_operator() else {
            return left;
        };
        let right = self.parse_value_atom();
        let span = Span::new(self.expression_value_start(left), self.value_end(right));
        self.push_expression(ExpressionValue::Comparison {
            left,
            operator,
            right,
            span,
        })
    }

    // Parse a unary condition and preserve its operand as an indexed AST value.
    fn parse_unary(&mut self) -> ValueId {
        let start = self.advance().span.start;
        let operand = self.parse_condition_expression();
        let span = Span::new(start, self.value_end(operand));
        self.push_expression(ExpressionValue::Unary {
            operator: crate::ast::UnaryOperator::Not,
            operand,
            span,
        })
    }

    // Parse parenthesized conditions while retaining the complete source span.
    fn parse_group(&mut self) -> ValueId {
        let open = self.advance();
        let nested = self.enter_nested(open.span);
        self.skip_separators();
        let expression = if nested {
            self.parse_condition_expression()
        } else {
            self.parse_error_value("maximum nesting depth exceeded")
        };
        self.skip_separators();
        if self.eat(TokenKind::RParen).is_none() {
            let span = self.current_span();
            self.diagnostics.push(Diagnostic::new(
                "E0501",
                "expected ')' after expression",
                span,
            ));
        }
        if nested {
            self.leave_nested();
        }
        self.push_expression(ExpressionValue::Group {
            expression,
            span: Span::new(open.span.start, self.value_end(expression)),
        })
    }

    // Parse a quantifier condition list with comma/newline separators treated equally.
    fn parse_quantifier(&mut self, kind: QuantifierKind) -> ValueId {
        let start = self.advance().span.start;
        let open = if let Some(token) = self.eat(TokenKind::LParen) {
            token
        } else {
            let span = self.current_span();
            self.diagnostics.push(Diagnostic::new(
                "E0502",
                "expected '(' after quantifier",
                span,
            ));
            Token {
                kind: TokenKind::LParen,
                span,
            }
        };
        let nested = self.enter_nested(open.span);
        let mut conditions = Vec::new();
        self.skip_separators();

        if nested {
            while !matches!(self.current_kind(), TokenKind::RParen | TokenKind::Eof) {
                let condition = self.parse_condition_expression();
                conditions.push(condition);
                if self.at(TokenKind::RParen) || self.at(TokenKind::Eof) {
                    break;
                }
                if !self.skip_separators() {
                    let span = self.current_span();
                    self.diagnostics.push(Diagnostic::new(
                        "E0503",
                        "expected ',' or newline between quantifier conditions",
                        span,
                    ));
                    self.recover_to_parenthesis();
                    break;
                }
            }
        } else {
            self.recover_to_parenthesis();
        }

        let close = self.eat(TokenKind::RParen).unwrap_or_else(|| {
            let span = self.current_span();
            self.diagnostics.push(Diagnostic::new(
                "E0504",
                "expected ')' after quantifier",
                span,
            ));
            Token {
                kind: TokenKind::RParen,
                span,
            }
        });
        if nested {
            self.leave_nested();
        }
        self.push_expression(ExpressionValue::Quantifier {
            kind,
            conditions,
            span: Span::new(start, close.span.end),
        })
    }

    fn current_quantifier(&self) -> Option<QuantifierKind> {
        if self.current_kind() != TokenKind::Identifier || self.peek_kind(1) != TokenKind::LParen {
            return None;
        }
        match self.text(self.current_span()) {
            "all" => Some(QuantifierKind::All),
            "any" => Some(QuantifierKind::Any),
            "one" => Some(QuantifierKind::One),
            "none" => Some(QuantifierKind::None),
            _ => None,
        }
    }

    fn is_unary_not_start(&self) -> bool {
        if self.current_kind() == TokenKind::Bang {
            return true;
        }
        if self.current_kind() != TokenKind::Identifier || self.text(self.current_span()) != "not" {
            return false;
        }
        !matches!(
            self.peek_kind(1),
            TokenKind::Comma
                | TokenKind::Newline
                | TokenKind::RParen
                | TokenKind::RBrace
                | TokenKind::Eof
        )
    }

    // Consume a supported comparison operator without accepting arbitrary identifiers.
    fn parse_comparison_operator(&mut self) -> Option<ComparisonOperator> {
        let operator = match self.current_kind() {
            TokenKind::LessThan => {
                self.advance();
                if self.eat(TokenKind::Equals).is_some() {
                    ComparisonOperator::LessEqual
                } else {
                    ComparisonOperator::Less
                }
            }
            TokenKind::GreaterThan => {
                self.advance();
                if self.eat(TokenKind::Equals).is_some() {
                    ComparisonOperator::GreaterEqual
                } else {
                    ComparisonOperator::Greater
                }
            }
            TokenKind::Identifier => match self.text(self.current_span()) {
                "less" => {
                    self.advance();
                    ComparisonOperator::Less
                }
                "more" => {
                    self.advance();
                    ComparisonOperator::Greater
                }
                "least" => {
                    self.advance();
                    ComparisonOperator::GreaterEqual
                }
                "most" => {
                    self.advance();
                    ComparisonOperator::LessEqual
                }
                "equals" => {
                    self.advance();
                    ComparisonOperator::Equals
                }
                "contains" => {
                    self.advance();
                    ComparisonOperator::Contains
                }
                "in" => {
                    self.advance();
                    ComparisonOperator::In
                }
                "matches" => {
                    self.advance();
                    ComparisonOperator::Matches
                }
                "starts" => {
                    self.advance();
                    ComparisonOperator::Starts
                }
                "ends" => {
                    self.advance();
                    ComparisonOperator::Ends
                }
                _ => return None,
            },
            _ => return None,
        };
        Some(operator)
    }

    // Store an expression and expose its source range through the flat CST index.
    fn push_expression(&mut self, expression: ExpressionValue) -> ValueId {
        let span = expression.span();
        let value_id = self.ast.push_value(Value::Expression(expression));
        self.push_cst(CstNodeKind::Expression, span, Vec::new());
        value_id
    }

    fn expression_value_start(&self, value_id: ValueId) -> u32 {
        self.value_span(value_id).start
    }

    fn value_span(&self, value_id: ValueId) -> Span {
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
            | Some(Value::Expression(crate::ast::ExpressionValue::Unary { span, .. }))
            | Some(Value::Expression(crate::ast::ExpressionValue::Comparison { span, .. }))
            | Some(Value::Expression(crate::ast::ExpressionValue::Group { span, .. }))
            | Some(Value::Expression(crate::ast::ExpressionValue::Quantifier { span, .. }))
            | Some(Value::Error(crate::ast::ErrorNode { span, .. })) => *span,
            None => self.current_span(),
        }
    }

    // Recover at the nearest enclosing delimiter without consuming the parent boundary.
    fn recover_to_parenthesis(&mut self) {
        let mut nested = 0_u32;
        while !matches!(
            self.current_kind(),
            TokenKind::Eof | TokenKind::RParen | TokenKind::RBrace
        ) {
            match self.current_kind() {
                TokenKind::LParen => nested = nested.saturating_add(1),
                TokenKind::RParen if nested > 0 => nested -= 1,
                _ => {}
            }
            self.advance();
        }
    }
}
