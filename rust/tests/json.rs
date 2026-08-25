// fon-parser/tests/json.rs

#![cfg(feature = "json")]

use fon_parser::{JsonConversionError, parse_to_json_value, to_json_string};
use serde_json::json;

#[test]
fn materializes_package_manifest_as_clean_json() {
    let source = "name = @fer/std\nversion = 0.1.0\nlicense = .mit\nauthors = [`Fuyeor`, `AI`]\ndescription = `The standard library`\ndependencies = {\n  @fer/common = ^0.1.0\n}\n";

    let actual = parse_to_json_value(source).expect("manifest should materialize");
    let expected = json!({
        "name": "@fer/std",
        "version": "0.1.0",
        "license": "mit",
        "authors": ["Fuyeor", "AI"],
        "description": "The standard library",
        "dependencies": {
            "@fer/common": "^0.1.0",
        },
    });

    assert_eq!(actual, expected);
    assert_eq!(
        to_json_string(source).expect("manifest should serialize"),
        r#"{"authors":["Fuyeor","AI"],"dependencies":{"@fer/common":"^0.1.0"},"description":"The standard library","license":"mit","name":"@fer/std","version":"0.1.0"}"#
    );
}

#[test]
fn materializes_json_scalars_and_root_arrays() {
    let actual = parse_to_json_value("[14, -2.5, true, false, `text`, .mit, @fer/std]")
        .expect("array should materialize");

    assert_eq!(
        actual,
        json!([14, -2.5, true, false, "text", "mit", "@fer/std"])
    );
}

#[test]
fn exports_schema_and_infers_default_type() {
    let source = "AppAppearance: struct {\n  mode: AppMode\n  primary-color: Hex = #AEA4E4\n  secondary-color: Hex = #ffe710\n  font-size: u8 = 14\n  enable-animations = true\n}\n";
    let parsed = fon_parser::parse(source);
    assert!(
        !parsed.has_errors(),
        "unexpected diagnostics: {:?}",
        parsed.diagnostics
    );

    let descriptor = parsed
        .document
        .schema_descriptor("AppAppearance")
        .expect("schema should export");
    let actual = serde_json::to_value(descriptor).expect("descriptor should serialize");
    let expected = json!({
        "name": "AppAppearance",
        "kind": "struct",
        "fields": [
            { "name": "mode", "type": "AppMode", "default": null, "required": true },
            { "name": "primary-color", "type": "Hex", "default": "#AEA4E4", "required": false },
            { "name": "secondary-color", "type": "Hex", "default": "#ffe710", "required": false },
            { "name": "font-size", "type": "u8", "default": 14, "required": false },
            { "name": "enable-animations", "type": "bool", "default": true, "required": false },
        ],
        "variants": [],
    });

    assert_eq!(actual, expected);
}

#[test]
fn strict_defaults_reject_required_fields_and_partial_defaults_omit_them() {
    let source = "AppAppearance: struct {\n  mode: AppMode\n  primary-color: Hex = #AEA4E4\n  font-size: u8 = 14\n  enable-animations = true\n}\n";
    let document = fon_parser::parse(source).document;

    assert_eq!(
        document.instantiate_defaults("AppAppearance"),
        Err(JsonConversionError::MissingRequiredField {
            schema: "AppAppearance".into(),
            field: "mode".into(),
        })
    );
    assert_eq!(
        document
            .materialize_partial_defaults("AppAppearance")
            .expect("partial defaults should materialize"),
        json!({
            "primary-color": "#AEA4E4",
            "font-size": 14,
            "enable-animations": true,
        })
    );
}

#[test]
fn materialization_rejects_diagnostics_duplicate_keys_and_interpolation() {
    assert!(matches!(
        parse_to_json_value("broken"),
        Err(JsonConversionError::ParseDiagnostics { .. })
    ));
    assert_eq!(
        parse_to_json_value("name = `first`\nname = `second`"),
        Err(JsonConversionError::DuplicateKey { key: "name".into() })
    );
    assert!(matches!(
        parse_to_json_value("value = `hello {name}`"),
        Err(JsonConversionError::UnsupportedInterpolation { .. })
    ));
}
