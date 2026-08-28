// typescript/test/serializer.spec.ts

import { describe, expect, it } from 'vitest';
import { FON, parse, rawAtom, stringify, text } from '../src/index.js';

describe('FON serializer', () => {
  it('serializes ordinary JavaScript data in pretty format by default', () => {
    const value = {
      name: 'Fuyeor',
      version: rawAtom('0.0.1'),
      dependencies: { xxx: rawAtom('yyy') },
    };
    expect(stringify(value)).toBe(
      'name = `Fuyeor`\nversion = 0.0.1\ndependencies = {\n  xxx = yyy\n}',
    );
  });

  it('serializes production data in compact format', () => {
    const value = {
      name: 'Fuyeor',
      version: rawAtom('0.0.1'),
      dependencies: { xxx: rawAtom('yyy') },
    };
    const source = 'name=`Fuyeor`,version=0.0.1,dependencies={xxx=yyy}';
    expect(stringify(value, { format: 'compact' })).toBe(source);
    expect(parse(source).hasErrors()).toBe(false);
  });

  it('supports nested values, escaping, and explicit serializer options', () => {
    const source = stringify(
      {
        enabled: true,
        count: 3,
        values: ['a', 'b'],
        message: 'line\\value`',
        pattern: /[a-z]+/giu,
      },
      {
        format: 'pretty',
        indent: '\t',
        lineEnding: '\r\n',
        trailingNewline: true,
      },
    );
    expect(source).toBe(
      'enabled = true\r\n' +
        'count = 3\r\n' +
        'values = [`a`, `b`]\r\n' +
        'message = `line\\\\value\\``\r\n' +
        'pattern = /[a-z]+/giu\r\n',
    );
    expect(parse(source).hasErrors()).toBe(false);
  });

  it('supports bigint and preserves explicit raw atoms', () => {
    expect(
      stringify({ count: 9007199254740993n, license: rawAtom('.mit') }),
    ).toBe('count = 9007199254740993\nlicense = .mit');
    expect(rawAtom('@fer/std').raw).toBe('@fer/std');
  });

  it('exposes the optional namespace facade and round-trips its output', () => {
    const source = FON.stringify({ name: 'Fuyeor' });
    const result = FON.parse(source);
    expect(result.hasErrors()).toBe(false);
    if (result.document.root.kind !== 'implicit-object')
      throw new Error('expected implicit object');
    const member =
      result.document.ast.members[result.document.root.members[0] ?? -1];
    if (member?.kind !== 'binding') throw new Error('expected binding');
    expect(text(result.document, member.key.raw)).toBe('name');
  });

  it('rejects ambiguous or unsafe JavaScript values', () => {
    expect(() => stringify({ value: null })).toThrow(TypeError);
    expect(() => stringify({ value: undefined })).toThrow(TypeError);
    expect(() => stringify({ value: Number.NaN })).toThrow(TypeError);
    expect(() => stringify({ 'not valid': 'value' })).toThrow(TypeError);
    expect(() => stringify(Object.create(null))).not.toThrow();

    const circular: { self?: unknown } = {};
    circular.self = circular;
    expect(() => stringify(circular)).toThrow(TypeError);
  });
});
