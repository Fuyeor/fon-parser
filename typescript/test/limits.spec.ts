// test/limits.spec.ts

import { describe, expect, it } from 'vitest';
import { parse, parseBytes } from '../src/index.js';

describe('FON parser limits and diagnostics', () => {
  it('bounds nesting depth', () => {
    const result = parse('value = [[[[0]]]]', { maxDepth: 2 });
    expect(result.hasErrors()).toBe(true);
    expect(
      result.diagnostics.some((diagnostic) => diagnostic.code === 'E0008'),
    ).toBe(true);
  });

  it('bounds lexical token count', () => {
    const result = parse('a = 1\nb = 2', { maxTokens: 4 });
    expect(result.hasErrors()).toBe(true);
    expect(
      result.diagnostics.some((diagnostic) => diagnostic.code === 'E0001'),
    ).toBe(true);
  });

  it('reports unterminated strings and block comments', () => {
    const stringResult = parse('value = `unterminated');
    expect(
      stringResult.diagnostics.some(
        (diagnostic) => diagnostic.code === 'E0004',
      ),
    ).toBe(true);
    const commentResult = parse('/* unterminated');
    expect(
      commentResult.diagnostics.some(
        (diagnostic) => diagnostic.code === 'E0003',
      ),
    ).toBe(true);
  });

  it('rejects invalid UTF-8 instead of replacing bytes', () => {
    expect(() => parseBytes(new Uint8Array([0xe7, 0x8e]))).toThrow(TypeError);
    expect(() => parseBytes(new Uint8Array([0x80]))).toThrow(TypeError);
  });

  it('fails fast for invalid options', () => {
    expect(() => parse('', { maxDepth: -1 })).toThrow(RangeError);
    expect(() => parse('', { maxTokens: Number.POSITIVE_INFINITY })).toThrow(
      RangeError,
    );
  });
});
