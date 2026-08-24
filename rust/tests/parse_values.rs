// fon-parser/tests/parse_values.rs

use fon_parser::{AstValue, SchemaKind, ValueKind, parse};

#[test]
fn parses_nested_objects_and_arrays() {
    let result = parse("authors = [`Fuyeor`, `AI`]\ndependencies = { @fer/common = ^0.1.0 }\n");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let members = result.document.ast.object_members().unwrap();
    assert_eq!(members.len(), 2);

    let authors = result
        .document
        .ast
        .member(members[0])
        .unwrap()
        .binding()
        .unwrap();
    assert_eq!(
        result.document.ast.value_kind(authors.value),
        ValueKind::Array
    );

    let dependencies = result
        .document
        .ast
        .member(members[1])
        .unwrap()
        .binding()
        .unwrap();
    assert_eq!(
        result.document.ast.value_kind(dependencies.value),
        ValueKind::Object
    );
}

#[test]
fn preserves_interpolation_parts_as_source_spans() {
    let result = parse("message = `Hello, {name}!`\n");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let member_id = result.document.ast.object_members().unwrap()[0];
    let binding = result
        .document
        .ast
        .member(member_id)
        .unwrap()
        .binding()
        .unwrap();
    let AstValue::String(string_value) = result.document.ast.value(binding.value).unwrap() else {
        panic!("expected string value");
    };

    assert_eq!(string_value.parts.len(), 3);
    assert!(string_value.parts[0].is_text());
    assert!(string_value.parts[1].is_interpolation());
    assert!(string_value.parts[2].is_text());
    assert_eq!(
        string_value.parts[0].text(result.document.source()),
        Some("Hello, ")
    );
    assert_eq!(
        string_value.parts[2].text(result.document.source()),
        Some("!")
    );
}

#[test]
fn distinguishes_schema_literals_from_runtime_objects() {
    let result = parse(
        "params = struct { username: string, age: u8 }\nvalue = { username = `alice`, age = 18 }\n",
    );

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let members = result.document.ast.object_members().unwrap();
    let schema_binding = result
        .document
        .ast
        .member(members[0])
        .unwrap()
        .binding()
        .unwrap();
    let object_binding = result
        .document
        .ast
        .member(members[1])
        .unwrap()
        .binding()
        .unwrap();

    assert!(matches!(
        result.document.ast.value(schema_binding.value).unwrap(),
        AstValue::Schema(schema) if schema.kind == SchemaKind::Struct
    ));
    assert_eq!(
        result.document.ast.value_kind(object_binding.value),
        ValueKind::Object
    );
}

#[test]
fn preserves_all_duplicate_members_in_source_order() {
    let result = parse("{ key = 1\nkey = 2\nkey = 3 }");

    assert!(
        !result.has_errors(),
        "duplicate keys are a semantic error, not a parser error"
    );
    let members = result.document.ast.object_members().unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!(result.document.ast.member_key_text(members[0]), Some("key"));
    assert_eq!(result.document.ast.member_key_text(members[1]), Some("key"));
    assert_eq!(result.document.ast.member_key_text(members[2]), Some("key"));
}
