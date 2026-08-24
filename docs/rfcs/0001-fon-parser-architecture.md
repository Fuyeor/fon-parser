<!-- docs/rfcs/0001-fon-parser-architecture.md -->

# RFC 0001: FON Parser Architecture

- **Status:** Draft
- **Authors:** Manus AI
- **License:** MIT
- **Package:** `fon-parser`
- **Rust crate:** `fon_parser`
- **Scope:** FON Core parser and optional scheme resolution

## Abstract

This RFC defines the long-term architecture of the official FON parser. FON is Fer Object Notation: a declarative object and data format related to the Fer programming language, but independently consumable by configuration tools, Webroamer, protocol implementations, editors, and other language ecosystems.

The implementation is an independent Rust crate with no dependency on Fer's high-level compiler, type checker, query database, or virtual file system. It may live in the `Fuyeor/fon-parser` repository and later be consumed by `Fuyeor/fer` through an adapter. The parser accepts in-memory `&str` or `&[u8]`, performs no external I/O and no evaluation, and produces a lossless concrete syntax tree plus a syntax AST containing raw `UnknownValue` nodes. An optional semantic phase resolves a scheme and produces typed HIR.

The architecture is intentionally split into:

```text
source bytes
    -> lexer
    -> lossless CST
    -> FON Syntax AST
    -> optional scheme resolution
    -> Typed HIR
    -> FerObject / Value or Webroamer document lowering
```

The parser is not a second Fer compiler. Fer source may contain richer references and executable expressions, while a standalone `.fon` file preserves unsupported or not-yet-typed atoms as safe `UnknownValue` nodes.

## 1. Motivation

The current `Fuyeor/fer` repository already separates compiler subsystems into workspace crates and its `syntax` crate uses an indexed, lossless CST with `NodeId(u32)`, source spans, checkpoint-based parsing, and error recovery.[1] [2] That implementation is a useful design reference, but it parses the complete Fer language. FON has a different root grammar and a smaller safety boundary.

The FON parser must therefore share principles and, where appropriate, low-level abstractions with Fer without sharing the complete parser state machine. In particular, FON requires all of the following:

| Requirement | Architectural consequence |
| --- | --- |
| Independent ecosystem use | The package must not depend on Fer's high-level compiler |
| Fer integration | Fer needs a small adapter from FON HIR to `FerObject`/`Value` |
| Lossless source truth | CST must preserve trivia, delimiters, comments, and raw spans |
| Honest unknown atoms | Bare non-string atoms must remain `UnknownValue` until a scheme resolves them |
| Arena memory model | All tree nodes use `NodeId(u32)` and flat indexed storage |
| TDD and maintainability | Tests and snapshots precede production parser code; modules remain small |
| No parser side effects | Parsing only consumes bytes and constructs data structures |
| Webroamer support | Webroamer uses a separate scheme/lowering layer, not parser name special cases |

## 2. Goals

This RFC has the following goals:

1. Define a stable independent Cargo package named `fon-parser`.
2. Define FON Core as a declarative data format and a declarative subset of Fer Object notation.
3. Support an implicit object root, an explicit object root, and a top-level array.
4. Support mixed newline and comma separators, including trailing separators.
5. Preserve every source byte required for lossless reprinting, diagnostics, migration, and IDE tooling.
6. Represent every duplicate member in source order and defer duplicate-key rejection to semantic analysis.
7. Support booleans, ordinary numbers, backtick strings, multiline strings, interpolation spans, arrays, objects, regex literals, enum shorthand, qualified enum values, schemas, type annotations, and safe unknown atoms.
8. Expose AST/HIR as the primary public interface and expose a complete `cst` module with visitor support.
9. Remain compatible with `core` and `alloc`, with optional `serde` support.
10. Provide explicit resource limits: `max_depth = 256` and `max_tokens = 1_000_000` by default.
11. Keep the first implementation whole-file based; incremental local repair is a later feature.
12. Make all source code, comments, diagnostics, module names, and paths English.

## 3. Non-goals

FON Core will not execute functions, resolve imports, read local or remote paths, access environment variables, compile regular expressions, load a Fer compiler database, or perform type checking during `parse()`. FON Core will not special-case Webroamer names such as `button`, `style`, or `on-click`. Those meanings belong to a scheme and a lowering layer.

Refinement types are intentionally out of scope for the first semantic implementation. The parser may later accept a generic constrained type syntax, but it must not invent a final syntax for refinement types before Fer defines whether the constraint belongs in `#[...]` or a type body.

## 4. Repository and package boundary

The first implementation is a standalone package in the `Fuyeor/fon-parser` repository. It is not initially embedded in `Fuyeor/fer` and it is not duplicated inside Fer. The repository may later contain implementations for TypeScript/JavaScript, Python, and Fer, but this RFC defines the Rust package boundary first.

```text
Fuyeor/fon-parser/
├── Cargo.toml
├── LICENSE
├── README.md
├── docs/
│   ├── grammar.md
│   └── rfcs/
│       └── 0001-fon-parser-architecture.md
└── rust/
    └── fon-parser/
        ├── Cargo.toml
        ├── src/
        └── tests/
```

The package location may be simplified to the repository root if the project chooses a single-language layout. The important invariant is that `fon-parser` is an independent Cargo package, not a private module inside Fer's `syntax` crate.

The dependency wall is:

```text
fon-parser
    -> core
    -> alloc
    -> optional serde

Fer adapter
    -> fon-parser
    -> Fer syntax / IR / analysis / query / vfs
```

The reverse dependency is forbidden:

```text
fon-parser -X-> Fer compiler
fon-parser -X-> Fer query database
fon-parser -X-> Fer VFS
fon-parser -X-> Webroamer runtime
```

## 5. FON profiles

The package shares one lexer and one syntax grammar across profiles, but semantic capabilities are explicit.

| Profile | Meaning | Allowed evaluation |
| --- | --- | --- |
| `Core` | Standalone data/configuration format | None |
| `FerObject` | FON-like object values embedded in Fer | Fer performs resolution/evaluation outside the parser |
| `WebroamerDocument` | Typed document object interpreted by a Webroamer scheme | Rendering/lowering only after scheme and capability checks |

`ParseOptions` chooses only syntactic policy and resource limits. A profile must not silently enable evaluation. A standalone `.fon` parse always preserves unknown names as safe raw atoms. Fer may parse richer expressions in its own language parser and then map the result to the same internal object model.

## 6. Syntax contract

### 6.1 Root

A file without root annotations may use an implicit root object, an explicit root object, or a top-level array. If one or more root annotations are present, the root body must be an explicit object.

```fon
name = `Fuyeor`
```

```fon
{ name = `Fuyeor` }
```

```fon
[1, 2, 3]
```

```fon
#[type = Manifest] {
  name = @fer/identifier
}
```

The CST must preserve whether braces were present. Semantic lowering may normalize an implicit and explicit object to the same `ObjectValue`, but formatting and migration must still know the original root form.

### 6.2 Members and separators

Members use the following shape:

```text
key [: Type] = value
```

Newlines and commas are both valid separators. They may be mixed and a trailing separator is valid.

```fon
{ a = 1, b = 2 }
```

```fon
{ a = 1
  b = 2 }
```

```fon
{ a = 1,
  b = 2,
}
```

Empty objects, arrays, structs, and enums are valid. The parser preserves every separator token. The canonical formatter may normalize mixed separators, while the lossless reprinter must reproduce the original bytes exactly.

### 6.3 Keys

All key shapes accepted by the FON grammar are preserved. The parser does not reject a key merely because it is not a local kebab-case identifier. Semantic validation may apply profile-specific rules.

The parser stores members in an ordered `Vec<MemberId>`. It never uses a map as the source of truth and never drops duplicate keys. Semantic analysis emits `DuplicateKeyError` when the same object contains the same resolved key more than once.

### 6.4 Values

FON supports the following syntax-level value categories:

```text
true / false
ordinary integer or decimal number
backtick string, including multiline form
array
object
/.../flags regular expression
.variant enum shorthand
Qualified.Enum.variant or Type.variant
struct schema literal
enum schema literal
unknown atom
```

Bare strings are forbidden. A textual value is a `StringLiteral` only when delimited by backticks. A non-string raw atom is never silently converted to a string.

### 6.5 Ordinary numbers and unknown atoms

A standard integer or decimal number is a `NumberValue`:

```fon
count = 100
ratio = 3.14
```

A token containing multiple dots or semantic punctuation is an `UnknownValue` with a lexical shape, for example:

```fon
version = 1.0.0
constraint = ^0.1.0
name = @fer/identifier
readme = ./docs/en.md
color = #AEA4E4
```

Unknown atoms end before a newline, comma, `}`, `]`, or a `//` comment. The lexer must retain the complete raw span. The parser may classify the shape as `VersionLike`, `PathLike`, `PackageLike`, `ColorLike`, `BareAtom`, or `Other`, but it must not assign a semantic type.

### 6.6 Strings and interpolation

The parser stores a lossless raw span and a sequence of string parts. Text parts reference source spans. Interpolation parts reference the source span of the expression and are not evaluated by FON Core.

```fon
message = `Hello, {name}!`
```

In FON Core, interpolation is represented but not evaluated. In Fer source, a Fer-level evaluation engine may evaluate the expression. The parser must not call that engine.

### 6.7 Regular expressions

The parser accepts `/pattern/flags`, including patterns such as:

```fon
identifier = /^[a-z0-9-]+$/i
```

The AST stores pattern and flag spans. Regex compilation and expensive validation happen only in a later semantic/runtime layer.

### 6.8 Types and schemas

The type grammar supports built-in types, named custom types, generic types, anonymous structs, anonymous enums, and ordinary typed fields. Built-in types are lowercase; custom types are conventionally uppercase.

```fon
name: string = `Fuyeor`
color: Hex = #AEA4E4
users: Array<string> = [`Fuyeor`, `AI`]
mode: Option<AppMode> = .dark
```

A struct field supports all three forms:

```fon
required: string
with-default = `guest`
typed-default: i32 = 100
```

A field written as `field = value` has a default value and an inferred type at semantic analysis time. The parser only records the syntactic distinction.

Both anonymous and named schemas are valid:

```fon
params = struct { username: string, age: u8 }
```

```fon
Message: enum {
  quit
  move: struct { x: i32, y: i32 }
  write: string
}
```

### 6.9 Enum values

When the expected enum type is known, shorthand is allowed:

```fon
mode: AppMode = .dark
```

A qualified form is allowed when a value is in a global or ambiguous context:

```fon
mode = AppMode.dark
```

The parser records both forms as enum-path syntax. Semantic resolution determines whether the path names a valid enum and variant.

### 6.10 Annotations

Annotations have the following forms:

```fon
#[test]
#[type = Manifest]
#[location = 0, interpolate = flat]
```

Annotation arguments reuse FON values. An annotation may attach to the document root, a binding, a struct field, an enum variant, or another declaration node permitted by the profile. `Manifest` in `#[type = Manifest]` is parsed as `UnknownValue(BareAtom)`.

The parser preserves annotation order, raw values, comments, separators, and spans. Annotation semantics are not hard-coded into the parser.

## 7. Module design

Every non-test Rust source file must have one primary responsibility and must remain below 400 lines after implementation. The first module plan is:

| Module | Responsibility | Must not do |
| --- | --- | --- |
| `source` | Borrowed/owned source bytes and source identity | Resolve Fer file paths |
| `span` | Byte positions, spans, line-independent ranges | Own compiler diagnostics |
| `token` | Token kinds, token records, trivia records | Parse grammar |
| `lexer` | UTF-8 byte scanning, comments, strings, regex, raw atoms | Resolve types or evaluate expressions |
| `cst` | Lossless indexed nodes and visitor trait | Perform semantic validation |
| `ast` | Public FON syntax AST handles and node views | Store runtime values |
| `parser` | Document/member/value/type/annotation parsing orchestration | Read files or query databases |
| `diagnostic` | Parser diagnostics and recovery metadata | Decide semantic validity |
| `scheme` | Resolver traits and typed-HIR interfaces | Depend on Fer compiler types |
| `hir` | Resolved typed values and semantic object views | Mutate external compiler state |
| `format` | Lossless reprint and canonical formatting | Evaluate values |
| `lib` | Public exports and feature gates | Hide core invariants |

Parser orchestration should be split further if needed:

```text
parser/document.rs
parser/member.rs
parser/value.rs
parser/type_expr.rs
parser/annotation.rs
parser/recovery.rs
```

The implementation must not create a monolithic `parser.rs` that combines lexing, AST definitions, semantic resolution, and formatting.

## 8. Memory and ownership model

All tree nodes use an integer ID and flat indexed storage. Recursive pointers such as `Box<Node>`, `Rc<Node>`, `Arc<Node>`, or borrowed `&Node` fields are forbidden as the source-of-truth representation.

The intended storage shape is:

```rust
pub struct SyntaxTree {
    pub source: Source,
    pub tokens: Vec<Token>,
    pub trivia: Vec<Trivia>,
    pub nodes: Vec<CstNode>,
    pub extra: Vec<ExtraData>,
    pub diagnostics: Vec<Diagnostic>,
}

pub struct NodeId(pub u32);
pub struct TokenId(pub u32);
```

A node may contain `NodeId`, `TokenId`, `Span`, `SymbolId`, or an index range into `extra`. Variable-length child collections are stored in flat ranges or vectors owned by the tree. The public API may offer checked views such as `node(id)`, but views must not replace indexed ownership.

The parser should borrow input whenever possible. Keys, raw atoms, string parts, regex patterns, and annotation text retain source spans in the CST. Decoded strings, interned symbols, typed colors, versions, and compiled regular expressions are created only by later phases that need them.

The first implementation must be safe to send across threads after parsing. It must avoid thread-local global interners and mutable global state. `Send + Sync` is an acceptance criterion for the immutable tree and AST/HIR containers where their field types allow it.

## 9. Public data model

The public AST is handle-based and profile-neutral at parse time.

```rust
pub struct Document {
    pub root: Root,
    pub syntax: SyntaxTree,
}

pub enum Root {
    ImplicitObject { members: Vec<MemberId> },
    ExplicitObject { members: Vec<MemberId> },
    Array { items: Vec<ValueId> },
}

pub enum Member {
    Binding(Binding),
    TypeDeclaration(TypeDeclaration),
    Error(ErrorNode),
}

pub struct Binding {
    pub annotations: Vec<AnnotationId>,
    pub key: Key,
    pub type_annotation: Option<TypeExprId>,
    pub value: ValueId,
}
```

The value layer is:

```rust
pub enum Value {
    Boolean(BooleanValue),
    Number(NumberValue),
    String(StringValue),
    Regex(RegexValue),
    EnumPath(EnumPathValue),
    Array(ArrayValue),
    Object(ObjectValue),
    Schema(SchemaValue),
    Unknown(UnknownValue),
    Error(ErrorNode),
}
```

`UnknownValue` is a first-class public node:

```rust
pub struct UnknownValue {
    pub raw: Span,
    pub shape: UnknownShape,
}

pub enum UnknownShape {
    BareAtom,
    PackageLike,
    PathLike,
    VersionLike,
    ColorLike,
    Other,
}
```

The AST must preserve a `NodeId` or source span for every public node so diagnostics, formatters, snapshots, and adapters can map semantic results back to source.

## 10. CST and visitor API

The `fon_parser::cst` module exposes the lossless CST and a read-only visitor interface.

```rust
pub trait Visitor {
    fn visit_document(&mut self, node: DocumentId) {}
    fn visit_member(&mut self, node: MemberId) {}
    fn visit_value(&mut self, node: ValueId) {}
    fn visit_type(&mut self, node: TypeExprId) {}
    fn visit_annotation(&mut self, node: AnnotationId) {}
}
```

The exact visitor surface may evolve, but the following invariants are fixed:

1. Visitors see source order.
2. Visitors can access node kind, span, token range, and children.
3. Visitors never mutate the tree through the default API.
4. A separate transformation API may be added later using a new tree or edit script.
5. Error nodes remain visible to visitors.

## 11. Parse and resolve API

The minimal API is:

```rust
#![no_std]

extern crate alloc;

pub fn parse(source: &str) -> ParseResult;
pub fn parse_bytes(source: &[u8]) -> ParseResult;
pub fn resolve(document: &Document, scheme: &dyn SchemeResolver) -> ResolveResult;
pub fn reprint_lossless(document: &Document) -> String;
pub fn format_canonical(document: &Document, options: FormatOptions) -> String;
```

The implementation may return an owned string through `alloc::string::String`; the API must not require a Fer `SourceMap` or a file path. A richer options form can be added without breaking the simple `parse(&str)` entry point:

```rust
pub struct ParseOptions {
    pub max_depth: u32,
    pub max_tokens: u32,
    pub max_token_length: u32,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            max_depth: 256,
            max_tokens: 1_000_000,
            max_token_length: 1_048_576,
        }
    }
}
```

`parse()` is pure with respect to external state. `resolve()` is read-only with respect to the document and scheme interface. It returns diagnostics and typed results; it does not mutate Fer IR.

## 12. Scheme resolution

The parser does not know Fer's type IDs. The package defines a small trait boundary:

```rust
pub trait SchemeResolver {
    fn resolve_type(&self, path: &NamePath) -> Result<TypeRef, SchemeError>;
    fn parse_unknown(
        &self,
        expected: Option<&TypeRef>,
        raw: RawAtom,
    ) -> Result<TypedAtom, SchemeError>;
    fn resolve_enum(
        &self,
        expected: Option<&TypeRef>,
        path: &NamePath,
    ) -> Result<EnumValue, SchemeError>;
}
```

A resolver may interpret `#AEA4E4` as `Hex`, `0.1.0` as a package version, or `./docs/en.md` as a path only when the declared type or scheme explicitly permits that interpretation. Without a resolver, those values remain unresolved raw atoms.

The semantic pipeline is:

```text
UnknownValue(raw, shape)
    -> expected type from binding/struct field/scheme
    -> resolver literal parser
    -> range/format/constraint validation
    -> TypedAtom or unresolved diagnostic
```

The first implementation does not include refinement type semantics. It may include type syntax nodes so that unsupported semantic features produce a precise diagnostic instead of being misparsed.

## 13. Diagnostics and recovery

The parser emits diagnostics and inserts explicit error nodes instead of silently dropping malformed input. Recovery synchronizes at the next newline or comma, and also respects `}`, `]`, and annotation delimiters.

Required first-wave recovery cases are:

| Input failure | Required result |
| --- | --- |
| Missing value after `=` | `ErrorNode` spanning the missing value site |
| Missing member separator | Diagnostic plus recovery at newline/comma/closing delimiter |
| Unclosed object/array | `ErrorNode` and end-of-input diagnostic |
| Invalid key | Diagnostic while preserving raw span |
| Invalid type expression | Type error node and continued member parsing |
| Excessive depth/tokens | One resource-limit diagnostic and bounded termination |

An error node is not a valid runtime value. HIR lowering must refuse to materialize a document containing unrecovered errors unless an explicit tolerant mode is introduced later.

## 14. Formatting and round-trip policy

Two operations are required because byte-identical reprinting and canonical normalization are different contracts.

### 14.1 Lossless reprint

`reprint_lossless()` reproduces the exact original source bytes represented by the CST, including comments, whitespace, newline/comma choice, trailing separators, and explicit versus implicit root braces. The primary round-trip test is:

```text
source -> parse -> reprint_lossless -> source
```

The comparison is byte-for-byte.

### 14.2 Canonical formatting

`format_canonical()` produces the project-approved normalized representation. It may normalize mixed separators, indentation, line endings, and spacing, but it must preserve semantic structure and raw unknown values. The canonical formatter must not evaluate or reinterpret `UnknownValue`.

This separation satisfies both requirements: the CST is the physical source truth, while formatter output can be made deterministic for commits and migration. Future Fer `fmt` and `migrate` transformations may use a structured edit script over the CST.

## 15. Serde feature

The default build has no serialization dependency. An optional Cargo feature enables serialization:

```toml
[features]
default = []
serde = ["dep:serde"]

[dependencies.serde]
version = "1"
optional = true
default-features = false
features = ["alloc"]
```

Serde representations are a transport/debugging view, not the source of truth. Node IDs, spans, token ranges, raw values, and error nodes must remain representable. Serialization must not replace the indexed internal storage.

## 16. Fer integration

Fer integrates through its own adapter and query layer. The adapter maps FON spans to a Fer `FileId`, converts FON diagnostics into Fer diagnostics, and lowers typed FON values into Fer's unified `FerObject`/`Value` representation.

```text
Fer source or .fon source
    -> source kind: Fon
    -> parse_fon(FileId) query
    -> resolve_fon(FileId, SchemeId) query
    -> lower_fon_value(FileId) query
    -> FerObject / Value
```

The FON parser remains unaware of `FileId`, `SourceMap`, Fer symbol tables, and Fer query IDs. Fer's query database owns caching and invalidation. The initial cache policy is whole-file hashing; local incremental repair is intentionally deferred.

Fer source may contain executable references and function calls. Those must remain in the Fer parser and evaluation pipeline. The FON adapter may consume the result of Fer evaluation, but standalone FON parsing must not acquire executable semantics.

## 17. Webroamer integration

Webroamer consumes FON AST/HIR through a document scheme. The parser treats the following as ordinary object members:

```fon
button = {
  style = { padding = 10 }
  on-click = noop
}
```

The `webroamer/document` scheme decides whether `button` becomes an element, `style` becomes a typed property, and `on-click` becomes a capability-checked symbol reference. The parser never recognizes those names as DOM nodes.

This keeps FON Core independent from a browser runtime and prevents configuration parsing from constructing mutable DOM objects, invoking callbacks, or accessing resources.

## 18. Safety and resource limits

The parser must be bounded against hostile input. The initial defaults are:

```text
max_depth  = 256
max_tokens = 1_000_000
```

The implementation should also bound maximum token length, total source size, maximum array/member count, annotation argument count, and regex raw length. The limits must be checked before allocation grows without bound. Parser recursion should be bounded by `max_depth`; an explicit stack may be used where it materially simplifies enforcement.

The lexer must not perform network I/O, filesystem I/O, environment access, dynamic code loading, regex compilation, or scheme loading. This is a pure parser library.

## 19. Test-first implementation plan

No production parser code may be written before the corresponding tests. The first commit after RFC approval should establish the test harness, fixtures, snapshot format, and module skeleton only where needed for compilation.

The tests are organized by contract:

```text
tests/
├── lex_literals.rs
├── lex_unknown_values.rs
├── lex_trivia.rs
├── parse_roots.rs
├── parse_members.rs
├── parse_values.rs
├── parse_types.rs
├── parse_annotations.rs
├── parse_errors.rs
├── format_lossless.rs
├── format_canonical.rs
├── ast_snapshots.rs
├── resolve_unknown_values.rs
└── fixtures/
    ├── basic.fon
    ├── mixed-separators.fon
    ├── unknown-values.fon
    ├── schemas.fon
    ├── annotations.fon
    └── invalid.fon
```

The first test matrix must include:

| Area | Required cases |
| --- | --- |
| Roots | implicit object, explicit object, annotated explicit object, top-level array |
| Separators | newline, comma, mixed, trailing comma, empty containers |
| Keys | local, package/path-like, unusual valid key, duplicate key preservation |
| Literals | bool, integer, decimal, backtick, multiline, interpolation spans, regex/flags |
| Unknown atoms | bare atom, package-like, path-like, version-like, color-like, comment boundary |
| Types | built-ins, named types, generics, anonymous struct/enum, defaults, enum payloads |
| Enum values | shorthand and qualified forms |
| Annotations | flag, named argument, multiple arguments, annotations on all permitted nodes |
| Errors | missing value, missing separator, unclosed delimiter, invalid type, resource limit |
| Formatting | byte-identical lossless reprint and deterministic canonical output |
| Threading | immutable tree `Send + Sync` compile-time assertions where applicable |

No test may hide an unimplemented feature by hard-coding the expected output without exercising the parser path. Unsupported features must have an explicit failing or diagnostic test until implemented.

## 20. Acceptance criteria for the first implementation milestone

The first implementation milestone is complete only when all of the following are true:

1. The crate builds with default features using `core` and `alloc` plus the selected minimal platform glue.
2. `parse(&str)` and `parse_bytes(&[u8])` parse the complete first-wave grammar.
3. The parser returns a lossless CST and public syntax AST with `NodeId(u32)` indexed storage.
4. `UnknownValue` preserves raw spans and lexical shapes without string coercion.
5. Duplicate members are preserved in source order and diagnosed during semantic resolution.
6. Structs, enums, defaults, generic types, enum shorthand, qualified enum values, and annotations have explicit AST nodes.
7. Parser diagnostics contain source spans and recovery never materializes malformed values as valid values.
8. Resource limits are enforced at lexer/parser boundaries.
9. `reprint_lossless()` is byte-identical for all valid and recoverable fixtures.
10. `format_canonical()` normalizes separators without changing semantic or raw-atom content.
11. The public CST visitor is available.
12. Serde support is optional and does not alter the default dependency graph.
13. The Fer adapter remains a separate follow-up change unless explicitly included in a later RFC.

## 21. Open implementation decisions after RFC approval

The architecture is frozen by this RFC. The following details may be selected during implementation without changing the public semantic boundary:

- whether `Span` uses `u32` or `u64` internally, provided limits and portability are explicit;
- whether the source owner is `String`, `Vec<u8>`, or an immutable source abstraction;
- whether variable-length child lists use `Range<u32>` into `extra` or dedicated typed arenas;
- whether the repository uses a root Cargo package or a `rust/fon-parser` package;
- the exact enum names of diagnostics and visitor convenience methods.

Any change to the root grammar, UnknownValue policy, side-effect boundary, duplicate-key policy, node ownership model, or Fer adapter direction requires a new RFC or an amendment to this RFC.

## References

[1]: https://github.com/Fuyeor/fer/tree/main/compiler-rs "Fuyeor/fer compiler-rs workspace layout"
[2]: https://github.com/Fuyeor/fer/tree/main/compiler-rs/syntax "Fuyeor/fer syntax crate README and source layout"
[3]: https://github.com/Fuyeor/fer/blob/main/compiler-rs/syntax/src/cst.rs "Current Fer lossless CST node model"
[4]: https://github.com/Fuyeor/fer/blob/main/compiler-rs/syntax/src/lex.rs "Current Fer lexer token and checkpoint model"
[5]: https://github.com/Fuyeor/fer/blob/main/compiler-rs/syntax/src/parse/mod.rs "Current Fer parser context and checkpoint model"
[6]: https://github.com/Fuyeor/fon-parser "FON parser repository baseline"
