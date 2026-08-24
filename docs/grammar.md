<!-- docs/grammar.md -->

# FON Grammar

This document defines the first implemented FON Core grammar. It is intentionally smaller than the Fer programming language grammar. Parsing is pure and produces a lossless CST plus a syntax AST; schemes may add semantic meaning later.

## Root forms

```ebnf
Document           ::= Trivia* Root Trivia* EOF
Root               ::= RootAnnotation* ExplicitObject
                     | RootAnnotation* Array
                     | RootAnnotation* ImplicitObject
ExplicitObject     ::= "{" Separator* MemberList? Separator* "}"
ImplicitObject     ::= MemberList
Array              ::= "[" Separator* ValueList? Separator* "]"
```

Root annotations require `ExplicitObject`; an annotated top-level array or implicit object is a parse error. Without annotations, an empty file is an empty implicit object.

## Members and separators

```ebnf
MemberList         ::= Member (Separator+ Member)* Separator*
ValueList          ::= Value (Separator+ Value)* Separator*
Separator          ::= Newline | ","
Member             ::= Annotation* Key TypeAnnotation? "=" Value
                     | Annotation* Key ":" Schema
TypeAnnotation     ::= ":" Type
```

Newlines and commas may be mixed. A trailing separator is valid. The parser records separators in CST trivia/token storage; canonical formatting may normalize them.

## Keys and values

```ebnf
Key                ::= Identifier | UnknownAtom
Value              ::= Boolean
                     | Number
                     | String
                     | Regex
                     | EnumPath
                     | Array
                     | ExplicitObject
                     | Schema
                     | UnknownAtom
Boolean            ::= "true" | "false"
Number             ::= Integer | Decimal
String             ::= BacktickString
Regex              ::= "/" RegexPattern "/" RegexFlags?
EnumPath           ::= "." Identifier | Identifier "." Identifier ("." Identifier)*
Schema             ::= "struct" StructBody | "enum" EnumBody
```

Bare text is not a string. A bare identifier in a value position is retained as `UnknownValue(BareAtom)`, unless it forms an enum path. The parser does not infer a semantic type from spelling alone.

`UnknownAtom` extends until whitespace, newline, comma, `}`, `]`, `{`, `[`, `(`, `)`, `:`, `=`, or a backtick. A `//` sequence ends an unknown atom before the comment. The parser records its lexical shape as `BareAtom`, `PackageLike`, `PathLike`, `VersionLike`, `ColorLike`, or `Other`.

A standard signed integer or decimal is a `NumberValue`. Values such as `1.0.0`, `^0.1.0`, `@fer/identifier`, `./docs/en.md`, and `#AEA4E4` are unknown atoms. Regex pattern and flags remain raw source-backed data and are not compiled by the parser.

## Strings

Backtick strings may span lines. Interpolation is represented as source spans and is not evaluated by FON Core.

```text
String ::= ` text ({ expression } text)* `
```

The current parser stores text and interpolation spans. Fer may evaluate interpolation in its own expression engine after lowering; standalone FON never executes interpolation.

## Types and schemas

```ebnf
Type               ::= TypeName GenericArguments?
                     | StructSchema
                     | EnumSchema
TypeName           ::= Identifier | UnknownAtom
GenericArguments   ::= "<" Type ("," Type)* ">"
StructSchema       ::= "struct" "{" StructFieldList? "}"
EnumSchema         ::= "enum" "{" EnumVariantList? "}"
StructField        ::= Annotation* Key (":" Type)? ("=" Value)?
EnumVariant        ::= Annotation* Key (":" Type)?
```

Built-in type names are conventionally lowercase, such as `string`, `bool`, `i32`, `u8`, `f64`, `Array<T>`, and `Option<T>`. Custom names are conventionally uppercase. The parser distinguishes spelling categories but does not perform type checking.

## Annotations

```ebnf
Annotation         ::= "#[" Identifier ("=" Value)? AnnotationArgument* "]"
AnnotationArgument ::= ","? (Key "=" Value | Value)
```

Examples:

```fon
#[required]
#[type = Manifest]
#[location = 0, interpolate = flat]
```

`Manifest` is a raw `UnknownValue(BareAtom)` at parse time. Annotation meanings are supplied by a scheme.

## Error recovery

Malformed members produce an error diagnostic and an indexed error node. Member-level recovery synchronizes at newline, comma, `}`, `]`, or end of input. An unrecovered error is never lowered to a valid runtime value.

## Resource limits

The default limits are:

```text
max_depth  = 256
max_tokens = 1_000_000
```

Implementations also bound token length. A limit violation terminates the corresponding scan or parse path and emits a structured diagnostic.
