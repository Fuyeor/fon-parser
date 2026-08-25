// fon-parser/tests/parse_expressions.rs

use fon_parser::{
    ComparisonOperator, CstNodeKind, ExpressionValue, QuantifierKind, Value, format_canonical,
    parse, reprint_lossless,
};

fn first_value(source: &str) -> (fon_parser::ParseResult, fon_parser::ValueId) {
    let result = parse(source);
    let member_id = result
        .document
        .ast
        .object_members()
        .expect("object root")
        .first()
        .copied()
        .expect("binding member");
    let value_id = result
        .document
        .ast
        .member(member_id)
        .expect("member")
        .binding()
        .expect("binding")
        .value;
    (result, value_id)
}

#[test]
fn parses_nested_quantifiers_with_mixed_separators() {
    let source = "can-access = all (\n  user.is-logged-in,\n  any (\n    user.role equals .admin\n    user.reputation >= 100\n  )\n  one (payment.is-credit-card, payment.is-crypto\n  )\n  none (\n    user.is-banned, user.is-suspended\n  )\n)\n";
    let (result, value_id) = first_value(source);

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(reprint_lossless(&result.document), source);
    assert!(
        result
            .document
            .cst
            .nodes
            .iter()
            .any(|node| node.kind == CstNodeKind::Expression)
    );
    assert!(
        result
            .document
            .cst
            .tokens
            .iter()
            .any(|token| token.kind == fon_parser::TokenKind::Comma)
    );
    assert!(
        result
            .document
            .cst
            .tokens
            .iter()
            .any(|token| token.kind == fon_parser::TokenKind::Newline)
    );

    let Value::Expression(ExpressionValue::Quantifier {
        kind: QuantifierKind::All,
        conditions,
        ..
    }) = result
        .document
        .ast
        .value(value_id)
        .expect("quantifier value")
    else {
        panic!("expected all quantifier");
    };
    assert_eq!(conditions.len(), 4);
    assert!(matches!(
        result.document.ast.value(conditions[1]),
        Some(Value::Expression(ExpressionValue::Quantifier {
            kind: QuantifierKind::Any,
            conditions,
            ..
        })) if conditions.len() == 2
    ));
    assert!(matches!(
        result.document.ast.value(conditions[2]),
        Some(Value::Expression(ExpressionValue::Quantifier {
            kind: QuantifierKind::One,
            conditions,
            ..
        })) if conditions.len() == 2
    ));
    assert!(matches!(
        result.document.ast.value(conditions[3]),
        Some(Value::Expression(ExpressionValue::Quantifier {
            kind: QuantifierKind::None,
            conditions,
            ..
        })) if conditions.len() == 2
    ));
}

#[test]
fn accepts_mixed_separators_in_enum_declarations() {
    let source = "Mode: enum { en, es\n  zh-hans, zh-hant\n}\n";
    let result = parse(source);

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let declaration = result
        .document
        .ast
        .member(result.document.ast.object_members().expect("object root")[0])
        .expect("declaration")
        .type_declaration()
        .expect("type declaration");
    let schema = result
        .document
        .ast
        .schema(declaration.definition)
        .expect("schema");
    assert_eq!(schema.variants.len(), 4);
}

#[test]
fn parses_all_quantifier_kinds_and_empty_lists() {
    let source = "all-condition = all ()\nany-condition = any ()\none-condition = one ()\nnone-condition = none ()\n";
    let result = parse(source);
    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let members = result.document.ast.object_members().expect("object root");
    let expected = [
        QuantifierKind::All,
        QuantifierKind::Any,
        QuantifierKind::One,
        QuantifierKind::None,
    ];
    for (member_id, expected_kind) in members.iter().zip(expected) {
        let value_id = result
            .document
            .ast
            .member(*member_id)
            .expect("member")
            .binding()
            .expect("binding")
            .value;
        assert!(matches!(
            result.document.ast.value(value_id),
            Some(Value::Expression(ExpressionValue::Quantifier {
                kind,
                conditions,
                ..
            })) if *kind == expected_kind && conditions.is_empty()
        ));
    }
}

#[test]
fn parses_comparison_operators_and_not_forms() {
    let source = "checks = all (\n  x < 1\n  x <= 2\n  x > 3\n  x >= 4\n  x less 5\n  x most 6\n  x more 7\n  x least 8\n  x equals .value\n  text contains `x`\n  value in [a, b]\n  text matches /x/\n  text starts `x`\n  text ends `z`\n  not (x > 0)\n  ! (x > -1)\n)\n";
    let (result, value_id) = first_value(source);
    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );

    let Value::Expression(ExpressionValue::Quantifier { conditions, .. }) = result
        .document
        .ast
        .value(value_id)
        .expect("quantifier value")
    else {
        panic!("expected quantifier");
    };
    let operators = conditions
        .iter()
        .filter_map(|condition| match result.document.ast.value(*condition) {
            Some(Value::Expression(ExpressionValue::Comparison { operator, .. })) => {
                Some(*operator)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        operators,
        vec![
            ComparisonOperator::Less,
            ComparisonOperator::LessEqual,
            ComparisonOperator::Greater,
            ComparisonOperator::GreaterEqual,
            ComparisonOperator::Less,
            ComparisonOperator::LessEqual,
            ComparisonOperator::Greater,
            ComparisonOperator::GreaterEqual,
            ComparisonOperator::Equals,
            ComparisonOperator::Contains,
            ComparisonOperator::In,
            ComparisonOperator::Matches,
            ComparisonOperator::Starts,
            ComparisonOperator::Ends,
        ]
    );
    assert!(matches!(
        result
            .document
            .ast
            .value(*conditions.get(14).expect("not condition")),
        Some(Value::Expression(ExpressionValue::Unary { .. }))
    ));
    assert!(matches!(
        result
            .document
            .ast
            .value(*conditions.get(15).expect("bang condition")),
        Some(Value::Expression(ExpressionValue::Unary { .. }))
    ));
}

#[test]
fn canonical_formatter_renders_expression_values() {
    let result = parse("condition = all (x > 1, not (y equals .blocked))\n");
    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        format_canonical(&result.document),
        "condition = all (\n  x > 1\n  not (y equals .blocked)\n)\n"
    );
}

#[test]
fn rejects_adjacent_conditions_without_a_separator() {
    let result = parse("condition = any (a > 1 b > 2)\n");

    assert!(result.has_errors());
    assert!(result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0503"
            && diagnostic.message == "expected ',' or newline between quantifier conditions"
    }));
}
