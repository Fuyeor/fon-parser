// fon-parser/tests/parse_types.rs

use fon_parser::{AstValue, EnumValueKind, TypeKind, parse};

#[test]
fn parses_builtin_named_and_generic_types() {
    let result = parse(
        "name: string = `Fuyeor`\nusers: Array<string> = [`Fuyeor`, `AI`]\nmode: Option<AppMode> = .dark\n",
    );

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let members = result.document.ast.object_members().unwrap();

    let name = result
        .document
        .ast
        .member(members[0])
        .unwrap()
        .binding()
        .unwrap();
    assert_eq!(
        result.document.ast.type_kind(name.type_annotation.unwrap()),
        TypeKind::Builtin
    );

    let users = result
        .document
        .ast
        .member(members[1])
        .unwrap()
        .binding()
        .unwrap();
    assert_eq!(
        result
            .document
            .ast
            .type_kind(users.type_annotation.unwrap()),
        TypeKind::Generic
    );

    let mode = result
        .document
        .ast
        .member(members[2])
        .unwrap()
        .binding()
        .unwrap();
    assert_eq!(
        result.document.ast.type_kind(mode.type_annotation.unwrap()),
        TypeKind::Generic
    );
}

#[test]
fn parses_struct_fields_with_required_inferred_and_typed_defaults() {
    let result = parse("User: struct { id: Uuid4 nickname = `guest` score: i32 = 100 }\n");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let declaration_id = result.document.ast.object_members().unwrap()[0];
    let declaration = result
        .document
        .ast
        .member(declaration_id)
        .unwrap()
        .type_declaration()
        .unwrap();
    let schema = result.document.ast.schema(declaration.definition).unwrap();

    assert_eq!(schema.fields.len(), 3);
    assert!(schema.fields[0].type_annotation.is_some());
    assert!(schema.fields[0].default_value.is_none());
    assert!(schema.fields[1].type_annotation.is_none());
    assert!(schema.fields[1].default_value.is_some());
    assert!(schema.fields[2].type_annotation.is_some());
    assert!(schema.fields[2].default_value.is_some());
}

#[test]
fn parses_enum_variants_with_optional_payload_types() {
    let result = parse("Message: enum { quit move: struct { x: i32, y: i32 } write: string }\n");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let declaration_id = result.document.ast.object_members().unwrap()[0];
    let declaration = result
        .document
        .ast
        .member(declaration_id)
        .unwrap()
        .type_declaration()
        .unwrap();
    let schema = result.document.ast.schema(declaration.definition).unwrap();

    assert_eq!(schema.variants.len(), 3);
    assert!(schema.variants[0].payload.is_none());
    assert!(schema.variants[1].payload.is_some());
    assert!(schema.variants[2].payload.is_some());
}

#[test]
fn preserves_enum_shorthand_and_qualified_paths() {
    let result = parse("mode: AppMode = .dark\nother = AppMode.light\n");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let members = result.document.ast.object_members().unwrap();

    for member_id in members {
        let binding = result
            .document
            .ast
            .member(*member_id)
            .unwrap()
            .binding()
            .unwrap();
        let AstValue::EnumPath(value) = result.document.ast.value(binding.value).unwrap() else {
            panic!("expected enum path");
        };
        assert!(matches!(
            value.kind,
            EnumValueKind::Shorthand | EnumValueKind::Qualified
        ));
    }
}

#[test]
fn parses_annotations_on_root_bindings_fields_and_variants() {
    let result = parse(
        "#[type = Manifest] { #[required] name: string = `Fuyeor` Config: struct { #[location = 0] color: Hex } Mode: enum { #[deprecated = true] old } }",
    );

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.document.ast.root_annotations().len(), 1);
    assert_eq!(result.document.ast.object_members().unwrap().len(), 3);
}
