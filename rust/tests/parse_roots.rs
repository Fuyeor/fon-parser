// fon-parser/tests/parse_roots.rs

use fon_parser::{RootKind, parse};

#[test]
fn parses_an_implicit_object_root() {
    let result = parse("name = `Fuyeor`\nversion = 1.0.0\n");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.document.ast.root_kind(), RootKind::ImplicitObject);
    assert_eq!(result.document.ast.object_members().unwrap().len(), 2);
}

#[test]
fn parses_an_explicit_object_root() {
    let result = parse("{ name = `Fuyeor` }");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.document.ast.root_kind(), RootKind::ExplicitObject);
    assert_eq!(result.document.ast.object_members().unwrap().len(), 1);
}

#[test]
fn parses_a_top_level_array() {
    let result = parse("[1, 2, 3]");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.document.ast.root_kind(), RootKind::Array);
    assert_eq!(result.document.ast.root_array_items().unwrap().len(), 3);
}

#[test]
fn requires_an_explicit_object_when_root_annotations_exist() {
    let result = parse("#[type = Manifest]\nname = `Fuyeor`\n");

    assert!(result.has_errors());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0101")
    );
}

#[test]
fn accepts_newline_comma_mixed_and_trailing_separators() {
    let sources = [
        "{ a = 1, b = 2 }",
        "{ a = 1\nb = 2 }",
        "{ a = 1,\n b = 2 }",
        "{ a = 1,\nb = 2, }",
    ];

    for source in sources {
        let result = parse(source);
        assert!(
            !result.has_errors(),
            "unexpected diagnostics for {source:?}: {:?}",
            result.diagnostics
        );
        assert_eq!(result.document.ast.object_members().unwrap().len(), 2);
    }
}

#[test]
fn accepts_empty_containers() {
    for source in ["{}", "[]", "{ values = [] }"] {
        let result = parse(source);
        assert!(
            !result.has_errors(),
            "unexpected diagnostics for {source:?}: {:?}",
            result.diagnostics
        );
    }
}
