// fon-parser/tests/parse_and_format.rs

use fon_parser::{format_canonical, parse, reprint_lossless};

#[test]
fn preserves_comments_and_source_bytes_in_lossless_reprint() {
    let source = "// leading\n{ a = 1,\n  // between\n  b = 2,\n}\n";
    let result = parse(source);

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(reprint_lossless(&result.document), source);
}

#[test]
fn canonical_formatting_normalizes_mixed_separators() {
    let source = "{ a = 1,\nb = 2, c = { d = 4 } }";
    let result = parse(source);

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        format_canonical(&result.document),
        "{\n  a = 1\n  b = 2\n  c = {\n    d = 4\n  }\n}\n"
    );
}

#[test]
fn canonical_formatting_preserves_annotations() {
    let result = parse("#[type = Manifest] { #[required] name = `Fuyeor` }");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        format_canonical(&result.document),
        "#[type = Manifest]\n{\n  #[required]\n  name = `Fuyeor`\n}\n"
    );
}

#[test]
fn canonical_formatting_keeps_an_implicit_root_unbraced() {
    let result = parse("name = `Fuyeor`\nversion = 1.0.0\n");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        format_canonical(&result.document),
        "name = `Fuyeor`\nversion = 1.0.0\n"
    );
}

#[test]
fn inserts_error_nodes_and_continues_after_a_missing_value() {
    let result = parse("{ first =\nsecond = 2\nthird = 3 }");

    assert!(result.has_errors());
    assert!(result.document.cst.has_error_nodes());
    assert_eq!(result.document.ast.object_members().unwrap().len(), 3);
}

#[test]
fn reports_invalid_utf8_from_byte_input() {
    let result = fon_parser::parse_bytes(b"name = `Fuyeor`\xff");

    assert!(result.has_errors());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0003")
    );
}

#[test]
fn rejects_an_oversized_document_before_unbounded_growth() {
    let result = fon_parser::parse_with_options(
        "a = 1\nb = 2\n",
        fon_parser::ParseOptions {
            max_depth: 256,
            max_tokens: 3,
            max_token_length: 1024,
        },
    );

    assert!(result.has_errors());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0002")
    );
}

#[test]
fn recovers_from_an_invalid_schema_without_stalling() {
    let result = parse("AppMode: enum = { dark, light }\nnext = true\n");

    assert!(result.has_errors());
    assert!(!result.document.ast.object_members().unwrap().is_empty());
}

#[test]
fn rejects_excessive_nesting_depth() {
    let result = fon_parser::parse_with_options(
        "value = { child = { grandchild = { leaf = true } } }",
        fon_parser::ParseOptions {
            max_depth: 2,
            max_tokens: 1_000_000,
            max_token_length: 1024,
        },
    );

    assert!(result.has_errors());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0001")
    );
}

#[test]
fn immutable_trees_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<fon_parser::Document>();
    assert_send_sync::<fon_parser::SyntaxTree>();
}
