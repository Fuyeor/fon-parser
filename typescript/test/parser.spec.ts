// test/parser.spec.ts

import { describe, expect, it } from 'vitest';
import { parse, parseBytes, text, visit } from '../src/index.js';

describe('FON parser', () => {
  it('parses implicit, explicit, and array roots', () => {
    const implicit = parse('name = `Fuyeor`\nversion = 1.0.0\n');
    expect(implicit.hasErrors()).toBe(false);
    expect(implicit.document.root.kind).toBe('implicit-object');
    expect(implicit.document.root.members).toHaveLength(2);

    const explicit = parse('{ name = `Fuyeor` }');
    expect(explicit.hasErrors()).toBe(false);
    expect(explicit.document.root.kind).toBe('explicit-object');

    const array = parse('[1, 2, 3]');
    expect(array.hasErrors()).toBe(false);
    expect(array.document.root.kind).toBe('array');
    expect(array.document.root.items).toHaveLength(3);
  });

  it('requires an explicit object for root annotations', () => {
    const result = parse('#[type = Manifest]\nname = `Fuyeor`\n');
    expect(result.hasErrors()).toBe(true);
    expect(
      result.diagnostics.some((diagnostic) => diagnostic.code === 'E0101'),
    ).toBe(true);
  });

  it('accepts mixed and trailing separators', () => {
    for (const source of [
      '{ a = 1, b = 2 }',
      '{ a = 1 , b = 2 }',
      '{ a = 1\nb = 2 }',
      '{ a = 1,\n b = 2 }',
      '{ a = 1,\nb = 2, }',
    ]) {
      const result = parse(source);
      expect(result.hasErrors(), source).toBe(false);
      expect(result.document.root.kind).toBe('explicit-object');
      if (result.document.root.kind === 'explicit-object')
        expect(result.document.root.members).toHaveLength(2);
    }
  });

  it('ignores line and block comments outside backtick strings', () => {
    const result = parse(
      '/* header */\na = @fer/std /* inline */\n// comment\nb = `// not a comment`\n',
    );
    expect(result.hasErrors()).toBe(false);
    if (result.document.root.kind !== 'implicit-object')
      throw new Error('expected implicit object');
    expect(result.document.root.members).toHaveLength(2);
  });

  it('preserves duplicate members and source-backed spans', () => {
    const source = '{ key = 1\nkey = 2\nkey = 3 }';
    const result = parse(source);
    expect(result.hasErrors()).toBe(false);
    if (result.document.root.kind !== 'explicit-object')
      throw new Error('expected explicit object');
    expect(result.document.root.members).toHaveLength(3);
    for (const memberId of result.document.root.members) {
      const member = result.document.ast.members[memberId];
      if (member?.kind !== 'binding') throw new Error('expected binding');
      expect(text(result.document, member.key.raw)).toBe('key');
    }
  });

  it('classifies values without coercing unknown atoms', () => {
    const result = parse(
      'package = @fer/std\nversion = 0.1.0\npath = ./docs/index.md\ncolor = #AEA4E4\nlicense = mit\n',
    );
    expect(result.hasErrors()).toBe(false);
    if (result.document.root.kind !== 'implicit-object')
      throw new Error('expected implicit object');
    const values = result.document.root.members.map((id) => {
      const member = result.document.ast.members[id];
      if (member?.kind !== 'binding') throw new Error('expected binding');
      return result.document.ast.values[member.value];
    });
    expect(values.map((value) => value?.kind)).toEqual([
      'unknown',
      'unknown',
      'unknown',
      'unknown',
      'unknown',
    ]);
    expect(
      values.map((value) => (value?.kind === 'unknown' ? value.shape : '')),
    ).toEqual([
      'package-like',
      'version-like',
      'path-like',
      'color-like',
      'bare-atom',
    ]);
  });

  it('retains string interpolation and regex spans', () => {
    const result = parse(
      'message = `Hello, {name}!`\nidentifier = /^[a-z0-9-]+$/i\n',
    );
    expect(result.hasErrors()).toBe(false);
    if (result.document.root.kind !== 'implicit-object')
      throw new Error('expected implicit object');
    const first =
      result.document.ast.members[result.document.root.members[0] ?? -1];
    const second =
      result.document.ast.members[result.document.root.members[1] ?? -1];
    if (first?.kind !== 'binding' || second?.kind !== 'binding')
      throw new Error('expected bindings');
    const string = result.document.ast.values[first.value];
    const regex = result.document.ast.values[second.value];
    expect(string?.kind).toBe('string');
    if (string?.kind === 'string') {
      expect(string.parts.map((part) => part.kind)).toEqual([
        'text',
        'interpolation',
        'text',
      ]);
      expect(
        text(
          result.document,
          string.parts[1]?.expression ?? { start: 0, end: 0 },
        ),
      ).toBe('name');
    }
    expect(regex?.kind).toBe('regex');
    if (regex?.kind === 'regex') {
      expect(text(result.document, regex.pattern)).toBe('^[a-z0-9-]+$');
      expect(text(result.document, regex.flags)).toBe('i');
    }
  });

  it('parses nested values, schemas, types, enums, and annotations', () => {
    const source = `#[type = Manifest]\n{\n  User: struct { #[required] id: Uuid4, nickname = \`guest\`, score: i32 = 100 }\n  Message: enum { quit, move: struct { x: i32, y: i32 }, write: string }\n  mode: Option<AppMode> = .dark\n  other = AppMode.light\n  params = struct { username: string, age: u8 }\n  value = { authors = [\`Fuyeor\`, \`AI\`] }\n}`;
    const result = parse(source);
    expect(result.hasErrors(), result.diagnostics).toBe(false);
    expect(result.document.root.kind).toBe('explicit-object');
    if (result.document.root.kind !== 'explicit-object')
      throw new Error('expected explicit object');
    expect(result.document.root.members).toHaveLength(6);
    const declarations = result.document.root.members
      .map((id) => result.document.ast.members[id])
      .filter((member) => member?.kind === 'type-declaration');
    expect(declarations).toHaveLength(2);
    const mode =
      result.document.ast.members[result.document.root.members[2] ?? -1];
    const other =
      result.document.ast.members[result.document.root.members[3] ?? -1];
    if (mode?.kind !== 'binding' || other?.kind !== 'binding')
      throw new Error('expected enum bindings');
    expect(result.document.ast.values[mode.value]?.kind).toBe('enum-path');
    expect(result.document.ast.values[other.value]?.kind).toBe('enum-path');
  });

  it('reports malformed input and enforces resource limits', () => {
    const malformed = parse('{ key = 1 value = 2');
    expect(malformed.hasErrors()).toBe(true);
    expect(
      malformed.diagnostics.some(
        (diagnostic) =>
          diagnostic.code === 'E0103' || diagnostic.code === 'E0205',
      ),
    ).toBe(true);

    const limited = parse('a = [1, 2, 3]', { maxCollectionItems: 2 });
    expect(limited.hasErrors()).toBe(true);
    expect(
      limited.diagnostics.some((diagnostic) => diagnostic.code === 'E0006'),
    ).toBe(true);
  });

  it('parses UTF-8 bytes without a runtime dependency', () => {
    const bytes = new Uint8Array([
      110, 97, 109, 101, 32, 61, 32, 96, 0xe7, 0x8e, 0xa5, 96,
    ]);
    const result = parseBytes(bytes);
    expect(result.hasErrors()).toBe(false);
  });

  it('visits indexed nodes in source order', () => {
    const result = parse('a = { b = [1] }\nc = 2');
    const seen: string[] = [];
    visit(result.document, {
      visitMember: (member) => {
        if (member.kind === 'binding')
          seen.push(text(result.document, member.key.raw));
      },
    });
    expect(seen).toEqual(['a', 'b', 'c']);
  });
});
