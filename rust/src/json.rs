// fon-parser/src/json.rs

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;
use core::str::FromStr;

use serde_json::{Map, Number, Value as JsonValue};

use crate::ast::{
    Member, Root, Schema, SchemaKind, StringPartKind, StructField, TypeExpr, TypeId, Value,
    ValueId, ValueKind,
};
use crate::{Diagnostic, Document, ParseResult, Span};

/// Errors raised while converting parsed FON data into business JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonConversionError {
    /// The source parsed with diagnostics and cannot be materialized strictly.
    ParseDiagnostics { diagnostics: Vec<Diagnostic> },
    /// A numeric literal is not representable as a JSON number.
    InvalidNumber { raw: String },
    /// A string contains interpolation that standalone FON cannot evaluate.
    UnsupportedInterpolation { span: Span },
    /// A value has no lossless business-JSON representation under this policy.
    UnsupportedValue { kind: ValueKind },
    /// A member kind cannot be represented as a data-object property.
    UnsupportedMember { kind: &'static str },
    /// A referenced AST node does not exist.
    InvalidReference { kind: &'static str, id: u32 },
    /// A business object contains duplicate keys.
    DuplicateKey { key: String },
    /// The requested named schema does not exist in the document root.
    SchemaNotFound { name: String },
    /// The requested schema is not a struct and cannot produce object defaults.
    SchemaIsNotStruct { name: String },
    /// A required struct field has no default value.
    MissingRequiredField { schema: String, field: String },
    /// JSON string serialization failed unexpectedly.
    Serialization(String),
}

impl fmt::Display for JsonConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseDiagnostics { diagnostics } => {
                write!(
                    formatter,
                    "FON source contains {} diagnostic(s)",
                    diagnostics.len()
                )
            }
            Self::InvalidNumber { raw } => write!(formatter, "invalid JSON number: {raw}"),
            Self::UnsupportedInterpolation { span } => write!(
                formatter,
                "string interpolation cannot be materialized at bytes {}..{}",
                span.start, span.end
            ),
            Self::UnsupportedValue { kind } => {
                write!(
                    formatter,
                    "FON value kind {kind:?} is not supported as business JSON"
                )
            }
            Self::UnsupportedMember { kind } => {
                write!(formatter, "FON member kind {kind} is not a data property")
            }
            Self::InvalidReference { kind, id } => {
                write!(formatter, "invalid {kind} reference: {id}")
            }
            Self::DuplicateKey { key } => write!(formatter, "duplicate object key: {key}"),
            Self::SchemaNotFound { name } => write!(formatter, "schema not found: {name}"),
            Self::SchemaIsNotStruct { name } => {
                write!(formatter, "schema is not a struct: {name}")
            }
            Self::MissingRequiredField { schema, field } => write!(
                formatter,
                "required field {field} in schema {schema} has no default value"
            ),
            Self::Serialization(message) => {
                write!(formatter, "JSON serialization failed: {message}")
            }
        }
    }
}

impl core::error::Error for JsonConversionError {}

/// A business-facing description of a parsed FON schema.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaDescriptor {
    /// The declaration name from the FON document.
    pub name: String,
    /// A stable lower-case schema kind such as `struct` or `enum`.
    pub kind: String,
    /// Struct fields in source order.
    pub fields: Vec<SchemaFieldDescriptor>,
    /// Enum variants in source order.
    pub variants: Vec<SchemaVariantDescriptor>,
}

/// A business-facing description of one struct field.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaFieldDescriptor {
    /// The original FON field key, including punctuation such as `primary-color`.
    pub name: String,
    /// The rendered type expression, if declared or inferred from a default value.
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub type_name: Option<String>,
    /// The materialized default, if the field declares one.
    pub default: Option<JsonValue>,
    /// Whether the field has no default and therefore requires an input value.
    pub required: bool,
}

/// A business-facing description of one enum variant.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaVariantDescriptor {
    /// The original FON variant name.
    pub name: String,
    /// The payload type, if the variant declares one.
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    pub type_name: Option<String>,
}

impl Document {
    /// Materialize the document root as clean business JSON.
    #[cfg(feature = "json")]
    pub fn to_json_value(&self) -> Result<JsonValue, JsonConversionError> {
        Materializer::new(self).root()
    }

    /// Materialize the document root as compact JSON text.
    #[cfg(feature = "json")]
    pub fn to_json_string(&self) -> Result<String, JsonConversionError> {
        let value = self.to_json_value()?;
        serde_json::to_string(&value)
            .map_err(|error| JsonConversionError::Serialization(error.to_string()))
    }

    /// Export one named root schema as a business-facing descriptor.
    #[cfg(feature = "json")]
    pub fn schema_descriptor(&self, name: &str) -> Result<SchemaDescriptor, JsonConversionError> {
        let schema = find_schema(self, name)?;
        SchemaExporter::new(self).export(name, schema)
    }

    /// Instantiate every defaulted field of a named struct, failing on required fields.
    #[cfg(feature = "json")]
    pub fn instantiate_defaults(&self, name: &str) -> Result<JsonValue, JsonConversionError> {
        let schema = find_schema(self, name)?;
        if schema.kind != SchemaKind::Struct {
            return Err(JsonConversionError::SchemaIsNotStruct { name: name.into() });
        }
        let mut object = Map::new();
        for field in &schema.fields {
            let Some(value_id) = field.default_value else {
                return Err(JsonConversionError::MissingRequiredField {
                    schema: name.into(),
                    field: field.key.raw.clone(),
                });
            };
            let value = Materializer::new(self).value(value_id)?;
            insert_unique(&mut object, field.key.raw.clone(), value)?;
        }
        Ok(JsonValue::Object(object))
    }

    /// Materialize only fields that declare defaults, omitting required fields.
    #[cfg(feature = "json")]
    pub fn materialize_partial_defaults(
        &self,
        name: &str,
    ) -> Result<JsonValue, JsonConversionError> {
        let schema = find_schema(self, name)?;
        if schema.kind != SchemaKind::Struct {
            return Err(JsonConversionError::SchemaIsNotStruct { name: name.into() });
        }
        let mut object = Map::new();
        for field in &schema.fields {
            let Some(value_id) = field.default_value else {
                continue;
            };
            let value = Materializer::new(self).value(value_id)?;
            insert_unique(&mut object, field.key.raw.clone(), value)?;
        }
        Ok(JsonValue::Object(object))
    }
}

impl ParseResult {
    /// Materialize a successfully parsed result as clean business JSON.
    #[cfg(feature = "json")]
    pub fn to_json_value(&self) -> Result<JsonValue, JsonConversionError> {
        ensure_no_diagnostics(self)?;
        self.document.to_json_value()
    }

    /// Materialize a successfully parsed result as compact JSON text.
    #[cfg(feature = "json")]
    pub fn to_json_string(&self) -> Result<String, JsonConversionError> {
        ensure_no_diagnostics(self)?;
        self.document.to_json_string()
    }
}

/// Parse FON and materialize its root as compact business JSON.
#[cfg(feature = "json")]
pub fn to_json_string(source: &str) -> Result<String, JsonConversionError> {
    parse_to_json_value(source).and_then(|value| {
        serde_json::to_string(&value)
            .map_err(|error| JsonConversionError::Serialization(error.to_string()))
    })
}

/// Parse FON and materialize its root as a business JSON value.
#[cfg(feature = "json")]
pub fn parse_to_json_value(source: &str) -> Result<JsonValue, JsonConversionError> {
    let parsed = crate::parse(source);
    ensure_no_diagnostics(&parsed)?;
    parsed.document.to_json_value()
}

fn ensure_no_diagnostics(result: &ParseResult) -> Result<(), JsonConversionError> {
    if result.diagnostics.is_empty() {
        return Ok(());
    }
    Err(JsonConversionError::ParseDiagnostics {
        diagnostics: result.diagnostics.clone(),
    })
}

struct Materializer<'document> {
    document: &'document Document,
    active_values: Vec<ValueId>,
}

impl<'document> Materializer<'document> {
    fn new(document: &'document Document) -> Self {
        Self {
            document,
            active_values: Vec::new(),
        }
    }

    fn root(mut self) -> Result<JsonValue, JsonConversionError> {
        match &self.document.ast.root {
            Root::ImplicitObject { members } | Root::ExplicitObject { members } => {
                self.object_members(members)
            }
            Root::Array { items } => {
                let mut values = Vec::with_capacity(items.len());
                for value_id in items {
                    values.push(self.value(*value_id)?);
                }
                Ok(JsonValue::Array(values))
            }
        }
    }

    fn object_members(
        &mut self,
        members: &[crate::ast::MemberId],
    ) -> Result<JsonValue, JsonConversionError> {
        let mut object = Map::new();
        for member_id in members {
            let member = self.document.ast.member(*member_id).ok_or(
                JsonConversionError::InvalidReference {
                    kind: "member",
                    id: member_id.0,
                },
            )?;
            let binding = match member {
                Member::Binding(binding) => binding,
                Member::TypeDeclaration(_) => {
                    return Err(JsonConversionError::UnsupportedMember {
                        kind: "type declaration",
                    });
                }
                Member::Error(_) => {
                    return Err(JsonConversionError::UnsupportedMember { kind: "error" });
                }
            };
            let value = self.value(binding.value)?;
            insert_unique(&mut object, binding.key.raw.clone(), value)?;
        }
        Ok(JsonValue::Object(object))
    }

    fn value(&mut self, value_id: ValueId) -> Result<JsonValue, JsonConversionError> {
        if self.active_values.contains(&value_id) {
            return Err(JsonConversionError::InvalidReference {
                kind: "cyclic value",
                id: value_id.0,
            });
        }
        let value =
            self.document
                .ast
                .value(value_id)
                .ok_or(JsonConversionError::InvalidReference {
                    kind: "value",
                    id: value_id.0,
                })?;
        self.active_values.push(value_id);
        let result = match value {
            Value::Boolean { value, .. } => Ok(JsonValue::Bool(*value)),
            Value::Number { raw, .. } => Number::from_str(raw)
                .map(JsonValue::Number)
                .map_err(|_| JsonConversionError::InvalidNumber { raw: raw.clone() }),
            Value::String(string) => self.string_value(string),
            Value::EnumPath(enum_value) => {
                let path = enum_value
                    .path
                    .strip_prefix('.')
                    .unwrap_or(&enum_value.path);
                Ok(JsonValue::String(path.into()))
            }
            Value::Array(array) => {
                let mut values = Vec::with_capacity(array.items.len());
                for item_id in &array.items {
                    values.push(self.value(*item_id)?);
                }
                Ok(JsonValue::Array(values))
            }
            Value::Object(object) => self.object_members(&object.members),
            Value::Unknown(unknown) => Ok(JsonValue::String(unknown.raw.clone())),
            Value::Regex(_) | Value::Schema(_) | Value::Error(_) => {
                Err(JsonConversionError::UnsupportedValue { kind: value.kind() })
            }
        };
        self.active_values.pop();
        result
    }

    fn string_value(
        &self,
        string: &crate::ast::StringValue,
    ) -> Result<JsonValue, JsonConversionError> {
        let mut output = String::new();
        for part in &string.parts {
            match part.kind {
                StringPartKind::Text => {
                    let text = self
                        .document
                        .source()
                        .get(part.span.start as usize..part.span.end as usize)
                        .ok_or(JsonConversionError::InvalidReference {
                            kind: "string span",
                            id: part.span.start,
                        })?;
                    output.push_str(text);
                }
                StringPartKind::Interpolation => {
                    return Err(JsonConversionError::UnsupportedInterpolation { span: part.span });
                }
            }
        }
        Ok(JsonValue::String(output))
    }
}

fn insert_unique(
    object: &mut Map<String, JsonValue>,
    key: String,
    value: JsonValue,
) -> Result<(), JsonConversionError> {
    if object.contains_key(&key) {
        return Err(JsonConversionError::DuplicateKey { key });
    }
    object.insert(key, value);
    Ok(())
}

fn find_schema<'document>(
    document: &'document Document,
    name: &str,
) -> Result<&'document Schema, JsonConversionError> {
    let members = document
        .ast
        .object_members()
        .ok_or(JsonConversionError::SchemaNotFound { name: name.into() })?;
    for member_id in members {
        let member =
            document
                .ast
                .member(*member_id)
                .ok_or(JsonConversionError::InvalidReference {
                    kind: "member",
                    id: member_id.0,
                })?;
        let Member::TypeDeclaration(declaration) = member else {
            continue;
        };
        if declaration.name.raw != name {
            continue;
        }
        let schema = document.ast.schema(declaration.definition).ok_or(
            JsonConversionError::InvalidReference {
                kind: "schema",
                id: declaration.definition.0,
            },
        )?;
        return Ok(schema);
    }
    Err(JsonConversionError::SchemaNotFound { name: name.into() })
}

struct SchemaExporter<'document> {
    document: &'document Document,
}

impl<'document> SchemaExporter<'document> {
    fn new(document: &'document Document) -> Self {
        Self { document }
    }

    fn export(&self, name: &str, schema: &Schema) -> Result<SchemaDescriptor, JsonConversionError> {
        let mut fields = Vec::with_capacity(schema.fields.len());
        for field in &schema.fields {
            fields.push(self.field_descriptor(field)?);
        }
        let mut variants = Vec::with_capacity(schema.variants.len());
        for variant in &schema.variants {
            variants.push(SchemaVariantDescriptor {
                name: variant.name.raw.clone(),
                type_name: variant.payload.and_then(|id| self.type_name(id)),
            });
        }
        Ok(SchemaDescriptor {
            name: name.into(),
            kind: schema_kind_name(schema.kind).into(),
            fields,
            variants,
        })
    }

    fn field_descriptor(
        &self,
        field: &StructField,
    ) -> Result<SchemaFieldDescriptor, JsonConversionError> {
        let default = field
            .default_value
            .map(|id| Materializer::new(self.document).value(id))
            .transpose()?;
        let type_name = field
            .type_annotation
            .and_then(|id| self.type_name(id))
            .or_else(|| default.as_ref().map(infer_json_type));
        Ok(SchemaFieldDescriptor {
            name: field.key.raw.clone(),
            type_name,
            required: field.default_value.is_none(),
            default,
        })
    }

    fn type_name(&self, type_id: TypeId) -> Option<String> {
        let type_expr = self.document.ast.types.get(type_id.0 as usize)?;
        Some(match type_expr {
            TypeExpr::Builtin { name, .. } => name.clone(),
            TypeExpr::Named { path, .. } => path.clone(),
            TypeExpr::Generic {
                constructor,
                arguments,
                ..
            } => {
                let arguments = arguments
                    .iter()
                    .filter_map(|id| self.type_name(*id))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{constructor}<{arguments}>")
            }
            TypeExpr::Schema { schema, .. } => format!("schema#{}", schema.0),
            TypeExpr::Error(_) => "error".into(),
        })
    }
}

fn schema_kind_name(kind: SchemaKind) -> &'static str {
    match kind {
        SchemaKind::Struct => "struct",
        SchemaKind::Enum => "enum",
    }
}

fn infer_json_type(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
    .into()
}
