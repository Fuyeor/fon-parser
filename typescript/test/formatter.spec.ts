// test/formatter.spec.ts

import { describe, expect, it } from 'vitest';
import { formatCanonical, parse, reprintLossless, text } from '../src/index.js';

describe('FON formatting', () => {
  it('reprints source byte-for-byte', () => {
    const source = '// header\n{a=1,\n b = `x, y` /* inline */}\n';
    const result = parse(source);
    expect(result.hasErrors()).toBe(false);
    expect(reprintLossless(result.document)).toBe(source);
  });

  it('normalizes separators without changing raw atom spelling', () => {
    const result = parse('b=1,a=0\nversion=1.0.0\n');
    expect(result.hasErrors()).toBe(false);
    expect(formatCanonical(result.document)).toBe(
      'b = 1\na = 0\nversion = 1.0.0\n',
    );
  });

  it('formats nested objects, schemas, and annotation arguments', () => {
    const source =
      '#[location = 0, interpolate = flat]\n{ config = { version = 1.0.0 } }';
    const result = parse(source);
    expect(result.hasErrors()).toBe(false);
    expect(formatCanonical(result.document)).toContain(
      '#[location = 0, interpolate = flat]',
    );
    expect(formatCanonical(result.document)).toContain('version = 1.0.0');
    expect(result.document.root.annotations).toHaveLength(1);
    const annotation =
      result.document.ast.annotations[
        result.document.root.annotations[0] ?? -1
      ];
    expect(annotation?.arguments).toHaveLength(2);
    expect(
      annotation?.arguments[1]?.key === null
        ? ''
        : text(
            result.document,
            annotation?.arguments[1]?.key.raw ?? { start: 0, end: 0 },
          ),
    ).toBe('interpolate');
  });
});
