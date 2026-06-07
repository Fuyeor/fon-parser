**FON (Fer Object Notation)** is a lightweight data representation format and a declarative subset of the **Fer Programming Language**. This repository contains official FON parser implementations across multiple programming languages, engineered for high-performance data interchange and elegant configuration management.

[![License: MIT](https://img.shields.io/badge/License-MIT-AEA4E4?style=flat-square)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-AEA4E4?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)

## Syntax

FON is designed to be human-readable, visually structured, and minimal.

- **Strings**: Defined exclusively with backticks: `` `string` ``
- **Numbers**: Defined as raw numeric values: `20`
- **Booleans**: Defined as logical values: `true` or `false`
- **Arrays**: Defined using square brackets: `[123, 456]`
- **Objects/Structs**: Defined using braces with key-value pairs: `{ key = value }`
- **Comments**: Supported via double slashes: `// Comment`

> [!IMPORTANT]
> By default, FON is a multi-line, two-dimensional format that uses newlines (`\n`) as delimiters. For one-dimensional (flattened) representations, commas (`,`) are used as separators. Note that flattened FON does not support comments.

## Schema & Type Validation

FON can operate as a standalone data format or integrate with a predefined Fer schema for strict type validation. In this mode, values are validated against Fer's robust type system, including enums, structs, and fixed-width primitives.

**Example Schema Definition:**

```fer
/// @/config/app-appearance.fer
{ Hex } = @fer/ueiby

AppMode: enum = { dark, light, contrast, auto }

AppAppearance: struct = {
  mode: AppMode
  primary-color: Hex = #AEA4E4
  secondary-color: Hex = #ffe710
  font-size: u8 = 14
  enable-animations = true
}

exports { AppMode, AppAppearance }
```

**Usage with Schema Validation:**

```fer
{ AppMode, AppAppearance } = @/config/app-appearance

appconf: AppAppearance {
  mode = AppMode.dark
  primary-color = #AEA4E4
  secondary-color = 100    // Error: Does not match Hex type
  font-size = -100         // Error: Does not match u8 (unsigned) type
}
```

*Note: While the FON parser extracts raw data, type validation against the schema is handled by the specific language implementation or the Fer compiler.*

## Examples

**Multi-line (Standard)**

```fer
name = @fer/std
version = 0.1.0
license = mit
authors = [`Fuyeor`, `AI`]
description = `The standard library`
dependencies = {
  @fer/common = ^0.1.0
}
readme = { en = ./docs/en.md, fr = ./docs/fr.md }
```

**One-dimensional (Flattened)**

```fer
name = @fer/std, version = 0.1.0, license = mit, authors = [`Fuyeor`, `AI`], dependencies = { @fer/common = ^0.1.0 }
```

## Language Implementations

- **[TypeScript/JavaScript](./typescript)**: Native support for Web, Node.js, and Deno.
- **[Rust](./rust)**: High-performance, zero-copy, memory-safe implementation.
- **[Python](./python)**: Clean and idiomatic parser for scripting and data science.