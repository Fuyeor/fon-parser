// fon-parser/src/format.rs

use crate::ast::{
    Ast, ComparisonOperator, Document, ExpressionValue, Member, QuantifierKind, Root, TypeExpr,
    Value,
};
use alloc::string::String;

/// Reprint the exact source owned by the parsed document.
pub fn reprint_lossless(document: &Document) -> String {
    document.source().into()
}

/// Format a parsed document into the canonical multiline representation.
pub fn format_canonical(document: &Document) -> String {
    let mut output = String::new();
    render_annotations(
        &document.ast,
        &document.ast.root_annotations,
        0,
        &mut output,
    );
    match &document.ast.root {
        Root::ImplicitObject { members } => {
            render_members(&document.ast, members, 0, &mut output);
        }
        Root::ExplicitObject { members } => {
            render_object(&document.ast, members, 0, &mut output);
            output.push('\n');
        }
        Root::Array { items } => {
            render_array(&document.ast, items, 0, &mut output);
            output.push('\n');
        }
    }
    output
}

fn render_members(ast: &Ast, members: &[crate::ast::MemberId], level: usize, output: &mut String) {
    for (index, member_id) in members.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        render_member(ast, *member_id, level, output);
    }
    if !members.is_empty() {
        output.push('\n');
    }
}

fn render_object(ast: &Ast, members: &[crate::ast::MemberId], level: usize, output: &mut String) {
    output.push('{');
    if !members.is_empty() {
        output.push('\n');
        for (index, member_id) in members.iter().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            render_member(ast, *member_id, level + 1, output);
        }
        output.push('\n');
        write_indent(level, output);
    }
    output.push('}');
}

fn render_member(ast: &Ast, member_id: crate::ast::MemberId, level: usize, output: &mut String) {
    let Some(member) = ast.member(member_id) else {
        return;
    };
    let annotations = match member {
        Member::Binding(binding) => binding.annotations.as_slice(),
        Member::TypeDeclaration(declaration) => declaration.annotations.as_slice(),
        Member::Error(_) => &[],
    };
    render_annotations(ast, annotations, level, output);
    write_indent(level, output);
    match member {
        Member::Binding(binding) => {
            output.push_str(binding.key.raw.as_str());
            if let Some(type_id) = binding.type_annotation {
                output.push_str(": ");
                render_type(ast, type_id, output);
            }
            output.push_str(" = ");
            render_value(ast, binding.value, level, output);
        }
        Member::TypeDeclaration(declaration) => {
            output.push_str(declaration.name.raw.as_str());
            output.push_str(": ");
            if let Some(schema) = ast.schema(declaration.definition) {
                render_schema(ast, schema, level, output);
            }
        }
        Member::Error(error) => {
            output.push_str("/* ");
            output.push_str(error.message.as_str());
            output.push_str(" */");
        }
    }
}

fn render_value(ast: &Ast, value_id: crate::ast::ValueId, level: usize, output: &mut String) {
    let Some(value) = ast.value(value_id) else {
        return;
    };
    match value {
        Value::Boolean { value, .. } => output.push_str(if *value { "true" } else { "false" }),
        Value::Number { raw, .. } => output.push_str(raw.as_str()),
        Value::String(value) => output.push_str(value.raw.as_str()),
        Value::Regex(value) => {
            output.push('/');
            output.push_str(value.pattern.as_str());
            output.push('/');
            if let Some(flags) = &value.flags {
                output.push_str(flags.as_str());
            }
        }
        Value::EnumPath(value) => output.push_str(value.path.as_str()),
        Value::Unknown(value) => output.push_str(value.raw.as_str()),
        Value::Array(value) => render_array(ast, &value.items, level, output),
        Value::Object(value) => render_object(ast, &value.members, level, output),
        Value::Schema(value) => {
            if let Some(schema) = ast.schema(value.schema) {
                render_schema(ast, schema, level, output);
            }
        }
        Value::Expression(expression) => render_expression(ast, expression, level, output),
        Value::Error(error) => {
            output.push_str("/* ");
            output.push_str(error.message.as_str());
            output.push_str(" */");
        }
    }
}

// Render expressions in a stable canonical form without changing lossless source replay.
fn render_expression(ast: &Ast, expression: &ExpressionValue, level: usize, output: &mut String) {
    match expression {
        ExpressionValue::Unary {
            operator: crate::ast::UnaryOperator::Not,
            operand,
            ..
        } => {
            if matches!(
                ast.value(*operand),
                Some(Value::Expression(ExpressionValue::Group { .. }))
            ) {
                output.push_str("not ");
                render_value(ast, *operand, level, output);
            } else {
                output.push_str("not (");
                render_value(ast, *operand, level, output);
                output.push(')');
            }
        }
        ExpressionValue::Group { expression, .. } => {
            output.push('(');
            render_value(ast, *expression, level, output);
            output.push(')');
        }
        ExpressionValue::Comparison {
            left,
            operator,
            right,
            ..
        } => {
            render_value(ast, *left, level, output);
            output.push(' ');
            output.push_str(comparison_text(*operator));
            output.push(' ');
            render_value(ast, *right, level, output);
        }
        ExpressionValue::Quantifier {
            kind, conditions, ..
        } => {
            output.push_str(quantifier_text(*kind));
            output.push_str(" (");
            for condition in conditions {
                output.push('\n');
                write_indent(level + 1, output);
                render_value(ast, *condition, level + 1, output);
            }
            if !conditions.is_empty() {
                output.push('\n');
                write_indent(level, output);
            }
            output.push(')');
        }
    }
}

fn comparison_text(operator: ComparisonOperator) -> &'static str {
    match operator {
        ComparisonOperator::Less => "<",
        ComparisonOperator::LessEqual => "<=",
        ComparisonOperator::Greater => ">",
        ComparisonOperator::GreaterEqual => ">=",
        ComparisonOperator::Equals => "equals",
        ComparisonOperator::Contains => "contains",
        ComparisonOperator::In => "in",
        ComparisonOperator::Matches => "matches",
        ComparisonOperator::Starts => "starts",
        ComparisonOperator::Ends => "ends",
    }
}

fn quantifier_text(kind: QuantifierKind) -> &'static str {
    match kind {
        QuantifierKind::All => "all",
        QuantifierKind::Any => "any",
        QuantifierKind::One => "one",
        QuantifierKind::None => "none",
    }
}

fn render_annotations(
    ast: &Ast,
    annotations: &[crate::ast::AnnotationId],
    level: usize,
    output: &mut String,
) {
    for annotation_id in annotations {
        let Some(annotation) = ast.annotation(*annotation_id) else {
            continue;
        };
        write_indent(level, output);
        output.push_str("#[");
        output.push_str(annotation.name.as_str());
        for (index, argument) in annotation.arguments.iter().enumerate() {
            if index == 0 {
                output.push_str(" = ");
            } else {
                output.push_str(", ");
            }
            if let Some(key) = &argument.key {
                output.push_str(key.raw.as_str());
                output.push_str(" = ");
            }
            if let Some(value_id) = argument.value {
                render_value(ast, value_id, level, output);
            }
        }
        output.push_str("]\n");
    }
}

fn render_array(ast: &Ast, items: &[crate::ast::ValueId], level: usize, output: &mut String) {
    output.push('[');
    for (index, value_id) in items.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        render_value(ast, *value_id, level, output);
    }
    output.push(']');
}

fn render_schema(ast: &Ast, schema: &crate::ast::Schema, level: usize, output: &mut String) {
    match schema.kind {
        crate::ast::SchemaKind::Struct => {
            output.push_str("struct {");
            for field in &schema.fields {
                output.push('\n');
                render_annotations(ast, &field.annotations, level + 1, output);
                write_indent(level + 1, output);
                output.push_str(field.key.raw.as_str());
                if let Some(type_id) = field.type_annotation {
                    output.push_str(": ");
                    render_type(ast, type_id, output);
                }
                if let Some(value_id) = field.default_value {
                    output.push_str(" = ");
                    render_value(ast, value_id, level + 1, output);
                }
            }
            if !schema.fields.is_empty() {
                output.push('\n');
                write_indent(level, output);
            }
            output.push('}');
        }
        crate::ast::SchemaKind::Enum => {
            output.push_str("enum {");
            for variant in &schema.variants {
                output.push('\n');
                render_annotations(ast, &variant.annotations, level + 1, output);
                write_indent(level + 1, output);
                output.push_str(variant.name.raw.as_str());
                if let Some(type_id) = variant.payload {
                    output.push_str(": ");
                    render_type(ast, type_id, output);
                }
            }
            if !schema.variants.is_empty() {
                output.push('\n');
                write_indent(level, output);
            }
            output.push('}');
        }
    }
}

fn render_type(ast: &Ast, type_id: crate::ast::TypeId, output: &mut String) {
    let Some(type_expr) = ast.types.get(type_id.0 as usize) else {
        return;
    };
    match type_expr {
        TypeExpr::Builtin { name, .. } | TypeExpr::Named { path: name, .. } => {
            output.push_str(name)
        }
        TypeExpr::Generic {
            constructor,
            arguments,
            ..
        } => {
            output.push_str(constructor.as_str());
            output.push('<');
            for (index, argument) in arguments.iter().enumerate() {
                if index > 0 {
                    output.push_str(", ");
                }
                render_type(ast, *argument, output);
            }
            output.push('>');
        }
        TypeExpr::Schema { schema, .. } => {
            if let Some(schema) = ast.schema(*schema) {
                render_schema(ast, schema, 0, output);
            }
        }
        TypeExpr::Error(error) => output.push_str(error.message.as_str()),
    }
}

fn write_indent(level: usize, output: &mut String) {
    for _ in 0..level {
        output.push_str("  ");
    }
}
