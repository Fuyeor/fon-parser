# FON Core conformance fixtures

This directory contains language-neutral fixtures for the FON Core parser. Rust and TypeScript runners read the same `manifest.json`, parse the same `input.fon`, project their implementation-specific ASTs into the same observable shape, and compare the result with `expected.json`.

The fixtures intentionally compare syntax-level behavior rather than private AST storage. They preserve root kind, member and array order, duplicate members, raw lexical values, unknown-atom shapes, string parts, regular-expression content, schema declarations, annotations, and stable error categories. They do not compare Node IDs, byte versus UTF-16 offsets, implementation-specific diagnostic codes, or localized diagnostic messages.

## Running the suite

Run the Rust suite with the `json` feature because its test-only projection uses `serde_json`:

```sh
cd rust
cargo test --features json --test conformance
```

Run the TypeScript suite from the package directory:

```sh
cd typescript
pnpm test
```

Both runners must pass before a fixture or parser change is merged. A fixture is normative test data, not a snapshot generated from either implementation; expected projections must be reviewed against the FON specification.

## Fixture layout

Each case directory contains an `input.fon` source file and an `expected.json` projection. `valid` cases require a successful parse and may also require lossless reprinting. `invalid` and `limits` cases require diagnostics and compare only the language-neutral `errorCategory` declared by the manifest.

The manifest uses `schemaVersion` to allow the fixture format to evolve independently from the parser implementations. Each case also records a `specRef` so a change can be reviewed against the corresponding FON specification section.

The current `fon-core` profile does not define serializer or binary-wire compatibility. Those should use separate fixture profiles after their specifications are stable.
