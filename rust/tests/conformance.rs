// rust/tests/conformance.rs

#![cfg(feature = "json")]

use std::fs;
use std::path::{Path, PathBuf};

use fon_parser::ast::{AnnotationArgument, Member, RootKind, Schema, TypeExpr, Value};
use fon_parser::{Document, MemberId, ParseOptions, ParseResult, Span, ValueId};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    kind: String,
    path: String,
    #[serde(rename = "errorCategory")]
    error_category: Option<String>,
    limits: Option<FixtureLimits>,
}

#[derive(Debug, Deserialize)]
struct FixtureLimits {
    #[serde(rename = "maxDepth")]
    max_depth: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ExpectedFixture {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    status: String,
    projection: Option<JsonValue>,
    #[serde(rename = "errorCategory")]
    error_category: Option<String>,
    lossless: Option<bool>,
}

#[test]
fn shared_core_fixtures_match_the_language_neutral_projection() {
    let root = fixture_root();
    let manifest: Manifest = read_json(&root.join("manifest.json"));

    for fixture in manifest.cases {
        let case_root = root.join(&fixture.path);
        let source = fs::read_to_string(case_root.join("input.fon")).unwrap();
        let expected: ExpectedFixture = read_json(&case_root.join("expected.json"));
        let result = match fixture.limits.as_ref().and_then(|limits| limits.max_depth) {
            Some(max_depth) => fon_parser::parse_with_options(
                &source,
                ParseOptions {
                    max_depth,
                    max_tokens: 1_000_000,
                    max_token_length: 1_048_576,
                },
            ),
            None => fon_parser::parse(&source),
        };

        assert_eq!(
            expected.schema_version, manifest.schema_version,
            "fixture {}",
            fixture.id
        );
        match fixture.kind.as_str() {
            "valid" => {
                assert_eq!(expected.status, "pass", "fixture {}", fixture.id);
                assert!(
                    !result.has_errors(),
                    "{}: {:?}",
                    fixture.id,
                    result.diagnostics
                );
                assert_eq!(
                    project_document(&result.document),
                    expected.projection.unwrap(),
                    "fixture {}",
                    fixture.id
                );
                if expected.lossless == Some(true) {
                    assert_eq!(
                        fon_parser::reprint_lossless(&result.document),
                        source,
                        "fixture {}",
                        fixture.id
                    );
                }
            }
            "invalid" | "limit" => {
                assert_eq!(expected.status, "error", "fixture {}", fixture.id);
                assert!(
                    result.has_errors(),
                    "fixture {} unexpectedly parsed",
                    fixture.id
                );
                let category = diagnostic_category(&result);
                assert_eq!(
                    Some(category),
                    fixture.error_category.as_deref(),
                    "fixture {}",
                    fixture.id
                );
                assert_eq!(
                    Some(category),
                    expected.error_category.as_deref(),
                    "fixture {}",
                    fixture.id
                );
            }
            kind => panic!("{}: unsupported fixture kind {kind}", fixture.id),
        }
    }
}

/// Locates the repository-level fixture corpus without adding runtime I/O to the parser crate.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/fon-core")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Converts implementation-specific AST storage into the shared fixture projection.
fn project_document(document: &Document) -> JsonValue {
    json!({ "root": project_root(document) })
}

fn project_root(document: &Document) -> JsonValue {
    let annotations = document
        .ast
        .root_annotations()
        .iter()
        .map(|id| project_annotation(document, *id))
        .collect::<Vec<_>>();
    match document.ast.root_kind() {
        RootKind::ImplicitObject => {
            let members = document.ast.object_members().unwrap();
            json!({
                "kind": "implicit-object",
                "annotations": annotations,
                "members": members.iter().map(|id| project_member(document, *id)).collect::<Vec<_>>(),
            })
        }
        RootKind::ExplicitObject => {
            let members = document.ast.object_members().unwrap();
            json!({
                "kind": "explicit-object",
                "annotations": annotations,
                "members": members.iter().map(|id| project_member(document, *id)).collect::<Vec<_>>(),
            })
        }
        RootKind::Array => {
            let items = document.ast.root_array_items().unwrap();
            json!({
                "kind": "array",
                "annotations": annotations,
                "items": items.iter().map(|id| project_value(document, *id)).collect::<Vec<_>>(),
            })
        }
    }
}

fn project_member(document: &Document, member_id: MemberId) -> JsonValue {
    let member = document.ast.member(member_id).unwrap();
    match member {
        Member::Binding(binding) => {
            let mut result = serde_json::Map::new();
            result.insert("kind".into(), json!("binding"));
            result.insert("key".into(), json!(binding.key.raw));
            result.insert(
                "annotations".into(),
                project_annotations(document, &binding.annotations),
            );
            if let Some(type_id) = binding.type_annotation {
                result.insert("type".into(), json!(project_type(document, type_id)));
            }
            result.insert("value".into(), project_value(document, binding.value));
            JsonValue::Object(result)
        }
        Member::TypeDeclaration(declaration) => {
            let schema = document.ast.schema(declaration.definition).unwrap();
            json!({
                "kind": "type-declaration",
                "key": declaration.name.raw,
                "annotations": project_annotations(document, &declaration.annotations),
                "schema": project_schema(document, schema),
            })
        }
        Member::Error(_) => json!({ "kind": "error-member", "annotations": [] }),
    }
}

fn project_annotations(document: &Document, ids: &[fon_parser::ast::AnnotationId]) -> JsonValue {
    JsonValue::Array(
        ids.iter()
            .map(|id| project_annotation(document, *id))
            .collect(),
    )
}

fn project_annotation(
    document: &Document,
    annotation_id: fon_parser::ast::AnnotationId,
) -> JsonValue {
    let annotation = document.ast.annotation(annotation_id).unwrap();
    let arguments = annotation
        .arguments
        .iter()
        .map(|argument| project_annotation_argument(document, argument))
        .collect::<Vec<_>>();
    json!({ "name": annotation.name, "arguments": arguments })
}

fn project_annotation_argument(document: &Document, argument: &AnnotationArgument) -> JsonValue {
    let key = argument.key.as_ref().map(|key| key.raw.clone());
    let value = argument
        .value
        .map(|value_id| project_value(document, value_id))
        .unwrap_or(JsonValue::Null);
    json!({ "key": key, "value": value })
}

fn project_value(document: &Document, value_id: ValueId) -> JsonValue {
    match document.ast.value(value_id).unwrap() {
        Value::Boolean { value, .. } => json!({ "kind": "boolean", "value": value }),
        Value::Number { raw, .. } => json!({
            "kind": "number",
            "raw": raw,
            "integer": !raw.contains('.') && !raw.contains('e') && !raw.contains('E'),
        }),
        Value::String(string) => json!({
            "kind": "string",
            "raw": text(document, string.span),
            "parts": string.parts.iter().map(|part| match part.kind {
                fon_parser::StringPartKind::Text => json!({ "kind": "text", "text": text(document, part.span) }),
                fon_parser::StringPartKind::Interpolation => json!({ "kind": "interpolation", "expression": interpolation_text(document, part.span) }),
            }).collect::<Vec<_>>(),
        }),
        Value::Regex(regex) => json!({
            "kind": "regex",
            "pattern": regex.pattern,
            "flags": regex.flags.as_deref().unwrap_or(""),
        }),
        Value::EnumPath(enum_value) => json!({
            "kind": "enum-path",
            "shorthand": matches!(enum_value.kind, fon_parser::EnumValueKind::Shorthand),
            "path": enum_value.path,
        }),
        Value::Array(array) => json!({
            "kind": "array",
            "items": array.items.iter().map(|id| project_value(document, *id)).collect::<Vec<_>>(),
        }),
        Value::Object(object) => json!({
            "kind": "object",
            "members": object.members.iter().map(|id| project_member(document, *id)).collect::<Vec<_>>(),
        }),
        Value::Schema(schema_value) => json!({
            "kind": "schema",
            "schema": project_schema(document, document.ast.schema(schema_value.schema).unwrap()),
        }),
        Value::Unknown(unknown) => json!({
            "kind": "unknown",
            "raw": unknown.raw,
            "shape": unknown_shape(unknown.shape),
        }),
        Value::Expression(_) => json!({ "kind": "expression" }),
        Value::Error(_) => json!({ "kind": "error" }),
    }
}

fn project_schema(document: &Document, schema: &Schema) -> JsonValue {
    json!({
        "schemaKind": match schema.kind {
            fon_parser::SchemaKind::Struct => "struct",
            fon_parser::SchemaKind::Enum => "enum",
        },
        "fields": schema.fields.iter().map(|field| json!({
            "key": field.key.raw,
            "type": field.type_annotation.map(|id| project_type(document, id)),
            "default": field.default_value.map(|id| project_value(document, id)),
        })).collect::<Vec<_>>(),
        "variants": schema.variants.iter().map(|variant| json!({
            "key": variant.name.raw,
            "type": variant.payload.map(|id| project_type(document, id)),
        })).collect::<Vec<_>>(),
    })
}

fn project_type(document: &Document, type_id: fon_parser::TypeId) -> String {
    let type_expr = document.ast.type_expr(type_id).unwrap();
    type_text(document, type_expr)
}

fn type_text(document: &Document, type_expr: &TypeExpr) -> String {
    let span = match type_expr {
        TypeExpr::Builtin { span, .. }
        | TypeExpr::Named { span, .. }
        | TypeExpr::Generic { span, .. }
        | TypeExpr::Schema { span, .. }
        | TypeExpr::Error(fon_parser::ast::ErrorNode { span, .. }) => *span,
    };
    text(document, span).to_owned()
}

fn text(document: &Document, span: Span) -> &str {
    document
        .source()
        .get(span.start as usize..span.end as usize)
        .unwrap()
}

fn interpolation_text(document: &Document, span: Span) -> String {
    let raw = text(document, span);
    raw.strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(raw)
        .to_owned()
}

fn unknown_shape(shape: fon_parser::UnknownShape) -> &'static str {
    match shape {
        fon_parser::UnknownShape::BareAtom => "bare-atom",
        fon_parser::UnknownShape::PackageLike => "package-like",
        fon_parser::UnknownShape::PathLike => "path-like",
        fon_parser::UnknownShape::VersionLike => "version-like",
        fon_parser::UnknownShape::ColorLike => "color-like",
        fon_parser::UnknownShape::Other => "other",
    }
}

fn diagnostic_category(result: &ParseResult) -> &'static str {
    let message = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    if message.contains("maximum nesting")
        || message.contains("resource limit")
        || message.contains("token limit")
    {
        return "resource-limit";
    }
    if message.contains("expected a value") || message.contains("missing value") {
        return "missing-value";
    }
    if message.contains("expected closing") || message.contains("unterminated") {
        return "unclosed-delimiter";
    }
    if message.contains("newline or comma") || message.contains("separator") {
        return "missing-separator";
    }
    "syntax-error"
}
