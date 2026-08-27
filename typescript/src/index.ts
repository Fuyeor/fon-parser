// src/index.ts

import { lex } from './lexer.js';
import { parseSource } from './parser.js';
import {
  Ast,
  defaultParseOptions,
  Diagnostic,
  Document,
  Member,
  ParseOptions,
  ParseResult,
  Span,
  Token,
  TokenKind,
  TypeExpression,
  Value,
  Visitor,
  text,
} from './types.js';

export * from './types.js';
export { lex } from './lexer.js';

/** Parses UTF-16 JavaScript text without I/O, evaluation, or runtime dependencies. */
export function parse(
  source: string,
  options: Partial<ParseOptions> = {},
): ParseResult {
  if (typeof source !== 'string')
    throw new TypeError('source must be a string');
  return parseSource(source, options);
}

/** Decodes UTF-8 bytes before parsing without adding a runtime dependency. */
export function parseBytes(
  source: Uint8Array,
  options: Partial<ParseOptions> = {},
): ParseResult {
  if (!(source instanceof Uint8Array))
    throw new TypeError('source must be a Uint8Array');
  return parseSource(decodeUtf8(source), options);
}

/** Decodes UTF-8 strictly without requiring DOM or Node ambient type declarations. */
function decodeUtf8(source: Uint8Array): string {
  let output = '';
  for (let index = 0; index < source.length;) {
    const first = source[index] ?? 0;
    let codePoint: number;
    let width: number;
    if (first <= 0x7f) {
      codePoint = first;
      width = 1;
    } else if (first >= 0xc2 && first <= 0xdf) {
      codePoint = first & 0x1f;
      width = 2;
    } else if (first >= 0xe0 && first <= 0xef) {
      codePoint = first & 0x0f;
      width = 3;
    } else if (first >= 0xf0 && first <= 0xf4) {
      codePoint = first & 0x07;
      width = 4;
    } else {
      throw new TypeError(`invalid UTF-8 leading byte at offset ${index}`);
    }
    if (index + width > source.length)
      throw new TypeError(`truncated UTF-8 sequence at offset ${index}`);
    for (let offset = 1; offset < width; offset += 1) {
      const continuation = source[index + offset] ?? 0;
      if ((continuation & 0xc0) !== 0x80)
        throw new TypeError(
          `invalid UTF-8 continuation byte at offset ${index + offset}`,
        );
      codePoint = (codePoint << 6) | (continuation & 0x3f);
    }
    if (
      (width === 2 && codePoint < 0x80) ||
      (width === 3 && codePoint < 0x800) ||
      (width === 4 && codePoint < 0x10000) ||
      codePoint > 0x10ffff ||
      (codePoint >= 0xd800 && codePoint <= 0xdfff)
    ) {
      throw new TypeError(`invalid UTF-8 code point at offset ${index}`);
    }
    output += String.fromCodePoint(codePoint);
    index += width;
  }
  return output;
}

/** Reprints the exact source retained by the lossless syntax tree. */
export function reprintLossless(document: Document): string {
  return document.source;
}

export interface FormatOptions {
  readonly indent?: string;
  readonly lineEnding?: string;
  readonly trailingNewline?: boolean;
}

/** Emits deterministic separators while preserving source-backed literal spelling. */
export function formatCanonical(
  document: Document,
  options: FormatOptions = {},
): string {
  const indent = options.indent ?? '  ';
  const lineEnding = options.lineEnding ?? '\n';
  const trailingNewline = options.trailingNewline ?? true;
  const output = formatRoot(document, indent, lineEnding, 0);
  return trailingNewline
    ? `${output}${output.endsWith(lineEnding) ? '' : lineEnding}`
    : output;
}

/** Visits indexed AST nodes in source order without mutating the document. */
export function visit(document: Document, visitor: Visitor): void {
  visitor.visitDocument?.(document);
  const root = document.ast.root;
  if (root.kind === 'array') {
    for (const value of root.items) visitValue(document, value, visitor);
    return;
  }
  for (const member of root.members) visitMember(document, member, visitor);
}

function visitMember(
  document: Document,
  memberId: number,
  visitor: Visitor,
): void {
  const member = document.ast.members[memberId];
  if (member === undefined) return;
  visitor.visitMember?.(member, memberId);
  for (const annotationId of member.annotations) {
    const annotation = document.ast.annotations[annotationId];
    if (annotation !== undefined)
      visitor.visitAnnotation?.(annotation, annotationId);
  }
  if (member.kind === 'binding') visitValue(document, member.value, visitor);
  else if (member.kind === 'type-declaration')
    visitType(document, member.schema, visitor);
}

function visitValue(
  document: Document,
  valueId: number,
  visitor: Visitor,
): void {
  const value = document.ast.values[valueId];
  if (value === undefined) return;
  visitor.visitValue?.(value, valueId);
  if (value.kind === 'array') {
    for (const item of value.items) visitValue(document, item, visitor);
  } else if (value.kind === 'object') {
    for (const member of value.members) visitMember(document, member, visitor);
  } else if (value.kind === 'schema') {
    for (const field of value.fields) {
      for (const annotationId of field.annotations) {
        const annotation = document.ast.annotations[annotationId];
        if (annotation !== undefined)
          visitor.visitAnnotation?.(annotation, annotationId);
      }
      if (field.typeAnnotation !== null)
        visitType(document, field.typeAnnotation, visitor);
      if (field.defaultValue !== null)
        visitValue(document, field.defaultValue, visitor);
    }
    for (const variant of value.variants) {
      for (const annotationId of variant.annotations) {
        const annotation = document.ast.annotations[annotationId];
        if (annotation !== undefined)
          visitor.visitAnnotation?.(annotation, annotationId);
      }
      if (variant.payload !== null)
        visitType(document, variant.payload, visitor);
    }
  }
}

function visitType(document: Document, typeId: number, visitor: Visitor): void {
  const type = document.ast.types[typeId];
  if (type === undefined) return;
  visitor.visitType?.(type, typeId);
  if (type.kind === 'generic')
    for (const argument of type.arguments)
      visitType(document, argument, visitor);
  else if (type.kind === 'schema') visitValue(document, type.schema, visitor);
}

function formatRoot(
  document: Document,
  indent: string,
  lineEnding: string,
  level: number,
): string {
  const root = document.ast.root;
  if (root.kind === 'array')
    return formatArray(document, root.items, indent, lineEnding, level);
  const body = formatMembers(
    document,
    root.members,
    indent,
    lineEnding,
    level + (root.kind === 'explicit-object' ? 1 : 0),
  );
  const annotations = root.annotations
    .map((id) => formatAnnotation(document, id, indent, lineEnding, level))
    .join(lineEnding);
  if (root.kind === 'explicit-object') {
    const object =
      root.members.length === 0
        ? '{}'
        : `{${lineEnding}${body}${lineEnding}${indent.repeat(level)}}`;
    return annotations.length === 0
      ? object
      : `${annotations}${lineEnding}${object}`;
  }
  return annotations.length === 0 ? body : `${annotations}${lineEnding}${body}`;
}

function formatMembers(
  document: Document,
  members: readonly number[],
  indent: string,
  lineEnding: string,
  level: number,
): string {
  return members
    .map(
      (memberId) =>
        `${indent.repeat(level)}${formatMember(document, memberId, indent, lineEnding, level)}`,
    )
    .join(lineEnding);
}

function formatMember(
  document: Document,
  memberId: number,
  indent: string,
  lineEnding: string,
  level: number,
): string {
  const member = document.ast.members[memberId];
  if (member === undefined) return '';
  const annotations = member.annotations
    .map((id) => formatAnnotation(document, id, indent, lineEnding, level))
    .join(` ${lineEnding}${indent.repeat(level)}`);
  const prefix =
    annotations.length > 0
      ? `${annotations}${lineEnding}${indent.repeat(level)}`
      : '';
  if (member.kind === 'error-member')
    return `${prefix}// error ${text(document, member.span)}`;
  const key = text(document, member.key.raw);
  if (member.kind === 'type-declaration')
    return `${prefix}${key}: ${formatType(document, member.schema, indent, lineEnding, level)}`;
  const type =
    member.typeAnnotation === null
      ? ''
      : `: ${formatType(document, member.typeAnnotation, indent, lineEnding, level)}`;
  return `${prefix}${key}${type} = ${formatValue(document, member.value, indent, lineEnding, level)}`;
}

function formatAnnotation(
  document: Document,
  annotationId: number,
  indent: string,
  lineEnding: string,
  level: number,
): string {
  const annotation = document.ast.annotations[annotationId];
  if (annotation === undefined) return '';
  const argumentsText = annotation.arguments
    .map((argument) => {
      const key =
        argument.key === null ? '' : `${text(document, argument.key.raw)} `;
      const equals = argument.equals ? '= ' : '';
      return `${key}${equals}${formatValue(document, argument.value, indent, lineEnding, level)}`;
    })
    .join(', ');
  return `#[${text(document, annotation.name)}${argumentsText.length > 0 ? ` ${argumentsText}` : ''}]`;
}

function formatValue(
  document: Document,
  valueId: number,
  indent: string,
  lineEnding: string,
  level: number,
): string {
  const value = document.ast.values[valueId];
  if (value === undefined) return '';
  if (value.kind === 'boolean') return value.value ? 'true' : 'false';
  if (
    value.kind === 'number' ||
    value.kind === 'unknown' ||
    value.kind === 'enum-path' ||
    value.kind === 'regex' ||
    value.kind === 'string'
  )
    return text(document, value.span);
  if (value.kind === 'array')
    return formatArray(document, value.items, indent, lineEnding, level);
  if (value.kind === 'object') {
    if (value.members.length === 0) return '{}';
    const body = formatMembers(
      document,
      value.members,
      indent,
      lineEnding,
      level + 1,
    );
    return `{${lineEnding}${body}${lineEnding}${indent.repeat(level)}}`;
  }
  if (value.kind !== 'schema') return text(document, value.span);
  if (value.schemaKind === 'struct') {
    const fields = value.fields
      .map((field) => {
        const key = text(document, field.key.raw);
        const type =
          field.typeAnnotation === null
            ? ''
            : `: ${formatType(document, field.typeAnnotation, indent, lineEnding, level + 1)}`;
        const defaultValue =
          field.defaultValue === null
            ? ''
            : ` = ${formatValue(document, field.defaultValue, indent, lineEnding, level + 1)}`;
        return `${indent.repeat(level + 1)}${key}${type}${defaultValue}`;
      })
      .join(`,${lineEnding}`);
    return fields.length === 0
      ? 'struct {}'
      : `struct {${lineEnding}${fields}${lineEnding}${indent.repeat(level)}}`;
  }
  const variants = value.variants
    .map(
      (variant) =>
        `${indent.repeat(level + 1)}${text(document, variant.name.raw)}${variant.payload === null ? '' : `: ${formatType(document, variant.payload, indent, lineEnding, level + 1)}`}`,
    )
    .join(`,${lineEnding}`);
  return variants.length === 0
    ? 'enum {}'
    : `enum {${lineEnding}${variants}${lineEnding}${indent.repeat(level)}}`;
}

function formatArray(
  document: Document,
  items: readonly number[],
  indent: string,
  lineEnding: string,
  level: number,
): string {
  return `[${items.map((item) => formatValue(document, item, indent, lineEnding, level)).join(', ')}]`;
}

function formatType(
  document: Document,
  typeId: number,
  indent: string,
  lineEnding: string,
  level: number,
): string {
  const type = document.ast.types[typeId];
  if (type === undefined) return '';
  if (type.kind === 'named') return text(document, type.name);
  if (type.kind === 'generic')
    return `${text(document, type.name)}<${type.arguments.map((argument) => formatType(document, argument, indent, lineEnding, level)).join(', ')}>`;
  if (type.kind === 'schema')
    return formatValue(document, type.schema, indent, lineEnding, level);
  return text(document, type.span);
}

export type {
  Ast,
  Diagnostic,
  Document,
  ParseOptions,
  ParseResult,
  Span,
  Token,
  TokenKind,
  TypeExpression,
  Value,
};

export { defaultParseOptions };
