// typescript/src/serializer.ts

const rawAtomTag = Symbol('fon-parser.raw-atom');
const invalidAtomCharacters = /[\s,{}[\]():=`]/u;

export interface RawAtom {
  readonly [rawAtomTag]: true;
  readonly raw: string;
}

export type StringifyFormat = 'pretty' | 'compact';

export interface StringifyOptions {
  readonly format?: StringifyFormat;
  readonly indent?: string;
  readonly lineEnding?: string;
  readonly trailingNewline?: boolean;
  readonly maxDepth?: number;
}

/** Creates an explicit source-backed FON atom without guessing string semantics. */
export function rawAtom(raw: string): RawAtom {
  if (
    typeof raw !== 'string' ||
    raw.length === 0 ||
    invalidAtomCharacters.test(raw)
  ) {
    throw new TypeError('rawAtom must contain one non-empty FON atom');
  }
  return Object.freeze({ [rawAtomTag]: true as const, raw });
}

/** Serializes JavaScript data into FON without evaluating user-provided values. */
export function stringify(
  value: unknown,
  options: StringifyOptions = {},
): string {
  const format = options.format ?? 'pretty';
  if (format !== 'pretty' && format !== 'compact')
    throw new RangeError(`unsupported FON format: ${format}`);
  const indent = options.indent ?? '  ';
  const lineEnding = options.lineEnding ?? '\n';
  const trailingNewline = options.trailingNewline ?? false;
  const maxDepth = options.maxDepth ?? 256;
  if (indent.includes('\r') || indent.includes('\n'))
    throw new TypeError('indent must not contain a line break');
  if (lineEnding !== '\n' && lineEnding !== '\r\n')
    throw new RangeError('lineEnding must be "\\n" or "\\r\\n"');
  if (!Number.isSafeInteger(maxDepth) || maxDepth < 0)
    throw new RangeError('maxDepth must be a non-negative safe integer');

  const seen = new Set<object>();
  const output = encodeValue(
    value,
    0,
    format,
    indent,
    lineEnding,
    maxDepth,
    seen,
    true,
  );
  return trailingNewline ? `${output}${lineEnding}` : output;
}

function encodeValue(
  value: unknown,
  depth: number,
  format: StringifyFormat,
  indent: string,
  lineEnding: string,
  maxDepth: number,
  seen: Set<object>,
  root: boolean,
): string {
  if (depth > maxDepth)
    throw new RangeError(`FON value nesting exceeds maxDepth ${maxDepth}`);
  if (isRawAtom(value)) return value.raw;
  if (typeof value === 'string') return quoteString(value);
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return encodeNumber(value);
  if (typeof value === 'bigint') return value.toString(10);
  if (value instanceof RegExp) return `/${value.source}/${value.flags}`;
  if (value === null)
    throw new TypeError(
      'FON has no null literal; use an explicit raw atom or omit the field',
    );
  if (value === undefined)
    throw new TypeError('undefined cannot be serialized to FON');
  if (typeof value === 'function' || typeof value === 'symbol')
    throw new TypeError(`unsupported FON value type: ${typeof value}`);
  if (typeof value !== 'object')
    throw new TypeError(`unsupported FON value type: ${typeof value}`);
  if (seen.has(value)) throw new TypeError('cannot serialize circular data');
  seen.add(value);
  try {
    if (Array.isArray(value))
      return encodeArray(
        value,
        depth,
        format,
        indent,
        lineEnding,
        maxDepth,
        seen,
      );
    return encodeObject(
      value,
      depth,
      format,
      indent,
      lineEnding,
      maxDepth,
      seen,
      root,
    );
  } finally {
    seen.delete(value);
  }
}

function encodeNumber(value: number): string {
  if (!Number.isFinite(value))
    throw new TypeError('FON numbers must be finite');
  if (Object.is(value, -0)) return '-0';
  return String(value);
}

function encodeArray(
  value: readonly unknown[],
  depth: number,
  format: StringifyFormat,
  indent: string,
  lineEnding: string,
  maxDepth: number,
  seen: Set<object>,
): string {
  const items: string[] = [];
  for (let index = 0; index < value.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (descriptor === undefined || !('value' in descriptor))
      throw new TypeError(`array index ${index} must be a data property`);
    items.push(
      encodeValue(
        descriptor.value,
        depth + 1,
        format,
        indent,
        lineEnding,
        maxDepth,
        seen,
        false,
      ),
    );
  }
  const separator = format === 'compact' ? ',' : ', ';
  return `[${items.join(separator)}]`;
}

function encodeObject(
  value: object,
  depth: number,
  format: StringifyFormat,
  indent: string,
  lineEnding: string,
  maxDepth: number,
  seen: Set<object>,
  root: boolean,
): string {
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null)
    throw new TypeError('only plain objects can be serialized to FON');
  const keys = Object.keys(value);
  if (keys.length === 0) return '{}';
  const entries: string[] = [];
  for (const key of keys) {
    if (!isValidKey(key))
      throw new TypeError(`invalid FON object key: ${JSON.stringify(key)}`);
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor === undefined || !('value' in descriptor))
      throw new TypeError(`object key ${key} must be a data property`);
    const encoded = encodeValue(
      descriptor.value,
      depth + 1,
      format,
      indent,
      lineEnding,
      maxDepth,
      seen,
      false,
    );
    entries.push(formatEntry(key, encoded, depth, format, indent, lineEnding));
  }
  if (root)
    return format === 'compact' ? entries.join(',') : entries.join(lineEnding);
  if (format === 'compact') return `{${entries.join(',')}}`;
  const body = entries.join(lineEnding);
  const prefix = indent.repeat(Math.max(depth - 1, 0));
  return `{${lineEnding}${body}${lineEnding}${prefix}}`;
}

function formatEntry(
  key: string,
  value: string,
  depth: number,
  format: StringifyFormat,
  indent: string,
  lineEnding: string,
): string {
  if (format === 'compact') return `${key}=${value}`;
  return `${indent.repeat(depth)}${key} = ${value}`;
}

function quoteString(value: string): string {
  return `\`${value.replaceAll('\\', '\\\\').replaceAll('`', '\\`')}\``;
}

function isValidKey(key: string): boolean {
  return key.length > 0 && !invalidAtomCharacters.test(key);
}

function isRawAtom(value: unknown): value is RawAtom {
  return typeof value === 'object' && value !== null && rawAtomTag in value;
}
