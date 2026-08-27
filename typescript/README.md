<!-- typescript/README.md -->

# fon-parser TypeScript

This directory contains the native TypeScript implementation of the FON (Fer Object Notation) parser. It is independent from the Rust implementation in `../rust` and has **no runtime dependencies**. The package performs parsing only: it does not access the filesystem or network, evaluate interpolation, compile regular expressions, resolve schemes, or execute arbitrary atoms.

## Requirements

The package targets modern ECMAScript and requires **Node.js 26.7.0 or newer**. The TypeScript compiler emits native ESM and does not downlevel the implementation for older runtimes.

## Install and build

From this directory, install the development dependencies and build the package:

```sh
pnpm install
pnpm run typecheck
pnpm run build
pnpm test
```

The published runtime surface is emitted to `dist/`. TypeScript, Vitest, and Prettier are development-only dependencies; the generated package has no runtime dependency graph.

## Basic usage

```ts
import {
  formatCanonical,
  parse,
  reprintLossless,
  text,
} from '@fuyeor/fon-parser';

const result = parse(`name = \`Fuyeor\`
version = 1.0.0
`);

if (result.hasErrors()) {
  for (const diagnostic of result.diagnostics) {
    console.error(diagnostic.code, diagnostic.message, diagnostic.span);
  }
} else {
  console.log(reprintLossless(result.document));
  console.log(formatCanonical(result.document));

  if (result.document.root.kind === 'implicit-object') {
    const firstMember =
      result.document.ast.members[result.document.root.members[0]];
    if (firstMember?.kind === 'binding') {
      console.log(text(result.document, firstMember.key.raw));
    }
  }
}
```

`parse()` accepts a JavaScript string and returns a `ParseResult`. `parseBytes()` accepts UTF-8 bytes and rejects malformed sequences instead of silently replacing them. Both functions are pure with respect to external state and retain source spans for diagnostics and tooling.

```ts
import { parseBytes } from '@fuyeor/fon-parser';

const source = new TextEncoder().encode('enabled = true');
const result = parseBytes(source);
```

## Serialize JavaScript values

`stringify()` is the functional serializer for ordinary JavaScript values. It uses `pretty` by default and emits an implicit FON object at the root. Use `format: 'compact'` for production output without optional whitespace.

```ts
import { rawAtom, stringify } from '@fuyeor/fon-parser';

const value = {
  name: 'Fuyeor',
  version: rawAtom('0.0.1'),
  dependencies: { xxx: rawAtom('yyy') },
};

stringify(value);
// name = `Fuyeor`
// version = 0.0.1
// dependencies = {
//   xxx = yyy
// }

stringify(value, { format: 'compact' });
// name=`Fuyeor`,version=0.0.1,dependencies={xxx=yyy}
```

JavaScript strings are always encoded as backtick strings. FON atoms such as package references, versions, paths, colors, and enum paths must be marked explicitly with `rawAtom()` so serialization never guesses semantic meaning from an ordinary string. `null`, `undefined`, non-finite numbers, functions, symbols, accessors, non-plain objects, sparse arrays, and circular data are rejected.

The optional namespace facade is also available when a grouped API is preferred:

```ts
import { FON } from '@fuyeor/fon-parser';

const result = FON.parse(FON.stringify({ enabled: true }));
```

## Public API

| API                                   | Purpose                                                  |
| ------------------------------------- | -------------------------------------------------------- |
| `parse(source, options?)`             | Parse a JavaScript string into a source-backed document. |
| `parseBytes(source, options?)`        | Strictly decode UTF-8 bytes and parse them.              |
| `stringify(value, options?)`          | Serialize JavaScript data as pretty or compact FON.      |
| `rawAtom(raw)`                        | Mark a string as an explicit FON atom.                   |
| `FON`                                 | Optional namespace facade for the stateless API.         |
| `lex(source, options?)`               | Inspect the lossless token stream and lexer diagnostics. |
| `reprintLossless(document)`           | Return the exact original source text.                   |
| `formatCanonical(document, options?)` | Emit deterministic normalized formatting.                |
| `visit(document, visitor)`            | Visit indexed AST nodes in source order.                 |
| `text(document, span)`                | Read text represented by a source span.                  |

The AST uses flat numeric indexes for members, values, types, and annotations. Unknown atoms remain source-backed `unknown` values with lexical shapes such as `package-like`, `version-like`, `path-like`, `color-like`, and `bare-atom`; they are never silently converted into strings or semantic values.

## Supported syntax

The parser supports implicit and explicit object roots, top-level arrays, mixed newline/comma separators, trailing separators, line and block comments, backtick strings including interpolation spans, regular expression literals, booleans, ordinary numbers, enum paths, nested arrays and objects, struct and enum schemas, generic type expressions, typed bindings, and annotations. Duplicate members are preserved in source order so semantic layers can diagnose them without losing source information.

Resource limits are explicit and configurable through `ParseOptions`. The defaults include a maximum nesting depth of 256, a maximum token count of 1,000,000, and bounded token, source, collection, and annotation sizes.

## Development notes

The implementation is intentionally written in native TypeScript using modern language features and the Node.js 26.7.0 runtime baseline. Formatting is configured by `./.prettierrc.json`; tests use Vitest and follow the `*.spec.ts` naming convention.
