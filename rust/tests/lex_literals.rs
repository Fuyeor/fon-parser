// fon-parser/tests/lex_literals.rs

use fon_parser::{UnknownShape, Value, parse};

fn binding_values(source: &str) -> Vec<Value> {
    let result = parse(source);
    assert!(
        !result.has_errors(),
        "unexpected parse errors: {:?}",
        result.diagnostics
    );
    let members = result.document.ast.object_members().expect("object root");
    members
        .iter()
        .map(|member_id| {
            let member = result.document.ast.member(*member_id).expect("member");
            let binding = member.binding().expect("binding");
            result
                .document
                .ast
                .value(binding.value)
                .expect("value")
                .clone()
        })
        .collect()
}

#[test]
fn parses_boolean_and_standard_numbers_as_known_values() {
    let values = binding_values(
        "enabled = true\ncount = 100\nratio = 3.14\nnegative = -100\npositive = +3.14\n",
    );

    assert!(matches!(values[0], Value::Boolean { value: true, .. }));
    assert!(matches!(values[1], Value::Number { .. }));
    assert!(matches!(values[2], Value::Number { .. }));
    assert!(matches!(values[3], Value::Number { .. }));
    assert!(matches!(values[4], Value::Number { .. }));
}

#[test]
fn preserves_semantic_atoms_as_unknown_values() {
    let values = binding_values(
        "version = 1.0.0\nconstraint = ^0.1.0\npath = ./docs/en.md\ncolor = #AEA4E4\nname = Fuyeor\n",
    );

    assert!(matches!(
        values[0],
        Value::Unknown(ref value) if value.shape == UnknownShape::VersionLike
    ));
    assert!(matches!(
        values[1],
        Value::Unknown(ref value) if value.shape == UnknownShape::VersionLike
    ));
    assert!(matches!(
        values[2],
        Value::Unknown(ref value) if value.shape == UnknownShape::PathLike
    ));
    assert!(matches!(
        values[3],
        Value::Unknown(ref value) if value.shape == UnknownShape::ColorLike
    ));
    assert!(matches!(
        values[4],
        Value::Unknown(ref value) if value.shape == UnknownShape::BareAtom
    ));
}

#[test]
fn preserves_regex_pattern_and_flags_without_compiling() {
    let values = binding_values("identifier = /^[a-z0-9-]+$/i\n");

    let Value::Regex(regex) = &values[0] else {
        panic!("expected regex value");
    };
    assert_eq!(regex.pattern, "^[a-z0-9-]+$");
    assert_eq!(regex.flags.as_deref(), Some("i"));
}
