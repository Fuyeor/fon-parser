# fon-parser

`fon-parser` is the independent Rust implementation of FON (Fer Object Notation). It parses in-memory FON source into a lossless indexed CST and a source-backed syntax AST, then optionally resolves unknown atoms through an application-provided scheme.

The crate does not depend on the Fer compiler, Fer query database, Fer VFS, Webroamer, the filesystem, or the network. Standalone FON parsing never evaluates interpolation or function references.

## Usage

```rust
use fon_parser::{format_canonical, parse, Value};

let result = parse("name = `Fuyeor`\nversion = 1.0.0\n");
assert!(!result.has_errors());

let canonical = format_canonical(&result.document);
assert_eq!(canonical, "name = `Fuyeor`\nversion = 1.0.0\n");

let members = result.document.ast.object_members().expect("object root");
let version = result.document.ast.member(members[1]).expect("member").binding().expect("binding");
assert!(matches!(
    result.document.ast.value(version.value),
    Some(Value::Unknown(_))
));
```

Enable serde support with:

```toml
fon-parser = { version = "0.0.0", features = ["serde"] }
```
