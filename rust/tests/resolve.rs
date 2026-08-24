// fon-parser/tests/resolve.rs

use fon_parser::{
    SchemeError, SchemeResolver, TypeReference, TypedAtom, TypedRoot, TypedValue, parse, resolve,
};

struct TestScheme;

impl SchemeResolver for TestScheme {
    fn resolve_type(&self, name: &str) -> Result<TypeReference, SchemeError> {
        Ok(TypeReference {
            name: name.into(),
            span: None,
        })
    }

    fn resolve_unknown(
        &self,
        expected_type: Option<&TypeReference>,
        value: &fon_parser::UnknownValue,
    ) -> Result<TypedAtom, SchemeError> {
        Ok(TypedAtom {
            type_name: expected_type
                .map(|type_reference| type_reference.name.clone())
                .unwrap_or_else(|| "Unknown".into()),
            raw: value.raw.clone(),
            span: value.span,
        })
    }
}

#[test]
fn resolves_unknown_values_using_the_declared_type() {
    let parsed = parse("color: Hex = #AEA4E4\n");
    assert!(
        !parsed.has_errors(),
        "unexpected diagnostics: {:?}",
        parsed.diagnostics
    );

    let resolved = resolve(&parsed.document, &TestScheme);
    assert!(
        !resolved.has_errors(),
        "unexpected diagnostics: {:?}",
        resolved.diagnostics
    );
    let TypedRoot::Object(members) = &resolved.document.root else {
        panic!("expected object root");
    };
    let member = members.first().expect("resolved member");
    let TypedValue::Atom(atom) = &member.value else {
        panic!("expected typed atom");
    };
    assert_eq!(atom.type_name, "Hex");
    assert_eq!(atom.raw, "#AEA4E4");
}

#[test]
fn reports_duplicate_keys_during_resolution() {
    let parsed = parse("{ key = 1\nkey = 2 }");
    assert!(!parsed.has_errors(), "duplicate keys are not parse errors");

    let resolved = resolve(&parsed.document, &TestScheme);
    assert!(resolved.has_errors());
    assert!(
        resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E1001")
    );
}
