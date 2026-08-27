// src/parser.ts

import { lex } from './lexer.js';
import {
  Annotation,
  AnnotationArgument,
  AnnotationId,
  ArrayValue,
  Ast,
  BindingMember,
  BooleanValue,
  defaultParseOptions,
  Diagnostic,
  Document,
  EnumPathValue,
  EnumVariant,
  ErrorMember,
  ErrorType,
  ErrorValue,
  ExplicitObjectRoot,
  GenericType,
  ImplicitObjectRoot,
  Key,
  Member,
  MemberId,
  NamedType,
  ObjectValue,
  ParseOptions,
  ParseResult,
  RegexValue,
  Root,
  SchemaType,
  SchemaValue,
  Span,
  StringPart,
  StringValue,
  StructField,
  Token,
  TokenKind,
  TypeDeclarationMember,
  TypeExpression,
  TypeId,
  UnknownShape,
  UnknownValue,
  Value,
  ValueId,
} from './types.js';

const identifierPattern = /^[A-Za-z_][A-Za-z0-9_-]*$/;
const numericPattern = /^[+-]?(?:\d+\.\d+|\d+|\.\d+)(?:[eE][+-]?\d+)?$/;
const enumSegmentPattern = /^[A-Za-z_][A-Za-z0-9_-]*$/;
const builtins = new Set([
  'bool',
  'string',
  'bytes',
  'char',
  'byte',
  'int',
  'float',
  'void',
  'never',
  'i8',
  'i16',
  'i32',
  'i64',
  'i128',
  'u8',
  'u16',
  'u32',
  'u64',
  'u128',
  'f32',
  'f64',
]);

/** Parses a token stream into source-backed flat AST storage. */
export function parseSource(
  source: string,
  suppliedOptions: Partial<ParseOptions> = {},
): ParseResult {
  const options = normalizeOptions(suppliedOptions);
  if (source.length > options.maxSourceLength) {
    const span = { start: options.maxSourceLength, end: source.length };
    const diagnostic: Diagnostic = {
      code: 'E0005',
      message: `source length exceeded (maximum ${options.maxSourceLength})`,
      severity: 'error',
      span,
    };
    const emptyRoot: ImplicitObjectRoot = {
      kind: 'implicit-object',
      annotations: [],
      members: [],
      span: { start: 0, end: source.length },
    };
    const emptyAst: Ast = {
      root: emptyRoot,
      members: [],
      values: [],
      types: [],
      annotations: [],
    };
    return new ParseResult({
      source,
      tokens: [],
      diagnostics: [diagnostic],
      ast: emptyAst,
      root: emptyRoot,
    });
  }
  const lexed = lex(source, options);
  const parser = new Parser(
    source,
    lexed.tokens,
    [...lexed.diagnostics],
    options,
  );
  return parser.parse();
}

function normalizeOptions(supplied: Partial<ParseOptions>): ParseOptions {
  const options = { ...defaultParseOptions, ...supplied };
  for (const [name, value] of Object.entries(options)) {
    if (!Number.isSafeInteger(value) || value < 0)
      throw new RangeError(`${name} must be a non-negative safe integer`);
  }
  return options;
}

class Parser {
  private readonly members: Member[] = [];
  private readonly values: Value[] = [];
  private readonly types: TypeExpression[] = [];
  private readonly annotations: Annotation[] = [];
  private readonly diagnostics: Diagnostic[];
  private tokenIndex = 0;
  private depth = 0;
  private resourceReported = false;

  public constructor(
    private readonly source: string,
    private readonly tokens: readonly Token[],
    diagnostics: Diagnostic[],
    private readonly options: ParseOptions,
  ) {
    this.diagnostics = diagnostics;
  }

  public parse(): ParseResult {
    const rootAnnotations = this.parseAnnotations();
    this.skipTrivia();
    let root: Root;
    const current = this.current().kind;
    if (current === TokenKind.LeftBrace) {
      const object = this.parseObject(true);
      root = {
        kind: 'explicit-object',
        annotations: rootAnnotations,
        members: object.members,
        span: object.span,
      } satisfies ExplicitObjectRoot;
    } else if (current === TokenKind.LeftBracket) {
      const array = this.parseArray();
      root = {
        kind: 'array',
        annotations: rootAnnotations,
        items: array.items,
        span: array.span,
      };
      if (rootAnnotations.length > 0)
        this.error(
          'E0101',
          'root annotations require an explicit object',
          root.span,
        );
    } else {
      const members = this.parseMemberList(TokenKind.Eof);
      root = {
        kind: 'implicit-object',
        annotations: rootAnnotations,
        members,
        span: { start: 0, end: this.source.length },
      } satisfies ImplicitObjectRoot;
      if (rootAnnotations.length > 0)
        this.error('E0101', 'root annotations require an explicit object', {
          start: 0,
          end: this.source.length,
        });
    }
    this.skipTrivia();
    if (this.current().kind !== TokenKind.Eof) {
      this.error(
        'E0102',
        'unexpected tokens after document root',
        this.current().span,
      );
    }
    const ast: Ast = {
      root,
      members: this.members,
      values: this.values,
      types: this.types,
      annotations: this.annotations,
    };
    const document: Document = {
      source: this.source,
      tokens: this.tokens,
      diagnostics: this.diagnostics,
      ast,
      root,
    };
    return new ParseResult(document);
  }

  private parseMemberList(endKind: TokenKind): MemberId[] {
    const memberIds: MemberId[] = [];
    this.skipTriviaAndSeparators();
    while (
      this.current().kind !== endKind &&
      this.current().kind !== TokenKind.Eof
    ) {
      if (this.resourceExceeded()) break;
      const before = this.tokenIndex;
      const member = this.parseMember();
      memberIds.push(member);
      const hadSeparator = this.consumeMemberBoundary(endKind);
      if (
        !hadSeparator &&
        this.current().kind !== endKind &&
        this.current().kind !== TokenKind.Eof
      ) {
        this.error(
          'E0103',
          'expected a newline or comma between members',
          this.current().span,
        );
      }
      if (this.tokenIndex === before) this.recover(endKind);
    }
    this.skipTriviaAndSeparators();
    return memberIds;
  }

  private parseMember(): MemberId {
    const start = this.current().span.start;
    const annotations = this.parseAnnotations();
    this.skipTrivia();
    const key = this.parseKey();
    if (key === null) {
      const end = this.recover(TokenKind.Eof);
      const member: ErrorMember = {
        kind: 'error-member',
        annotations,
        span: { start, end },
      };
      return this.addMember(member);
    }
    this.skipTrivia();
    if (this.consume(TokenKind.Colon)) {
      this.skipTrivia();
      const type = this.parseType();
      this.skipInlineTrivia();
      if (this.consume(TokenKind.Equals)) {
        this.skipTrivia();
        const value = this.parseValue();
        const member: BindingMember = {
          kind: 'binding',
          annotations,
          key,
          typeAnnotation: type,
          value,
          span: { start, end: this.valueEnd(value) },
        };
        return this.addMember(member);
      }
      const typeNode = this.types[type];
      if (typeNode !== undefined && typeNode.kind === 'schema') {
        const member: TypeDeclarationMember = {
          kind: 'type-declaration',
          annotations,
          key,
          schema: type,
          span: { start, end: this.typeEnd(type) },
        };
        return this.addMember(member);
      }
      this.error(
        'E0104',
        "expected '=' after a typed member",
        this.current().span,
      );
      const member: ErrorMember = {
        kind: 'error-member',
        annotations,
        span: { start, end: this.typeEnd(type) },
      };
      return this.addMember(member);
    }
    if (!this.consume(TokenKind.Equals)) {
      this.error(
        'E0105',
        "expected ':' or '=' after member key",
        this.current().span,
      );
      const end = this.recover(TokenKind.Eof);
      return this.addMember({
        kind: 'error-member',
        annotations,
        span: { start, end },
      });
    }
    this.skipTrivia();
    const value = this.parseValue();
    return this.addMember({
      kind: 'binding',
      annotations,
      key,
      typeAnnotation: null,
      value,
      span: { start, end: this.valueEnd(value) },
    });
  }

  private parseKey(): Key | null {
    const token = this.current();
    if (token.kind !== TokenKind.Atom) {
      this.error('E0106', 'expected a member key', token.span);
      return null;
    }
    this.advance();
    return { raw: token.span };
  }

  private parseValue(): ValueId {
    const token = this.current();
    switch (token.kind) {
      case TokenKind.String:
        return this.parseString();
      case TokenKind.Regex:
        return this.parseRegex();
      case TokenKind.LeftBracket:
        return this.addValue(this.parseArray());
      case TokenKind.LeftBrace:
        return this.addValue(this.parseObject(false));
      case TokenKind.Atom:
        return this.parseAtomValue();
      default:
        this.error('E0201', 'expected a value', token.span);
        if (token.kind !== TokenKind.Eof) this.advance();
        return this.addValue({
          kind: 'error',
          code: 'E0201',
          span: token.span,
        });
    }
  }

  private parseAtomValue(): ValueId {
    const token = this.current();
    const raw = this.source.slice(token.span.start, token.span.end);
    this.advance();
    if (raw === 'true' || raw === 'false') {
      return this.addValue({
        kind: 'boolean',
        value: raw === 'true',
        span: token.span,
      });
    }
    if (numericPattern.test(raw)) {
      return this.addValue({
        kind: 'number',
        raw: token.span,
        integer: !raw.includes('.') && !/[eE]/.test(raw),
        span: token.span,
      });
    }
    if (raw === 'struct' || raw === 'enum') {
      return this.addValue(this.parseSchema(raw, token.span.start));
    }
    if (isEnumPath(raw)) {
      const segments = raw.startsWith('.')
        ? raw.slice(1).split('.')
        : raw.split('.');
      const segmentStart = token.span.start + (raw.startsWith('.') ? 1 : 0);
      const spans: Span[] = [];
      let offset = segmentStart;
      for (const segment of segments) {
        spans.push({ start: offset, end: offset + segment.length });
        offset += segment.length + 1;
      }
      return this.addValue({
        kind: 'enum-path',
        shorthand: raw.startsWith('.'),
        segments: spans,
        span: token.span,
      });
    }
    const unknown: UnknownValue = {
      kind: 'unknown',
      raw: token.span,
      shape: classifyUnknown(raw),
      span: token.span,
    };
    return this.addValue(unknown);
  }

  private parseString(): ValueId {
    const token = this.current();
    this.advance();
    const parts: StringPart[] = [];
    const start = token.span.start + 1;
    const end = Math.max(
      start,
      token.span.end - (this.source[token.span.end - 1] === '`' ? 1 : 0),
    );
    let cursor = start;
    let textStart = start;
    while (cursor < end) {
      if (this.source[cursor] === '\\') {
        cursor += 2;
        continue;
      }
      if (this.source[cursor] !== '{') {
        cursor += 1;
        continue;
      }
      if (cursor > textStart)
        parts.push({ kind: 'text', span: { start: textStart, end: cursor } });
      const expressionStart = cursor + 1;
      let expressionEnd = expressionStart;
      let braceDepth = 1;
      while (expressionEnd < end) {
        const expressionCharacter = this.source[expressionEnd];
        if (expressionCharacter === '\\') {
          expressionEnd += 2;
          continue;
        }
        if (expressionCharacter === '{') braceDepth += 1;
        if (expressionCharacter === '}') {
          braceDepth -= 1;
          if (braceDepth === 0) break;
        }
        expressionEnd += 1;
      }
      if (braceDepth !== 0) {
        this.error('E0202', 'unterminated string interpolation', {
          start: cursor,
          end,
        });
        parts.push({
          kind: 'interpolation',
          span: { start: cursor, end },
          expression: { start: expressionStart, end },
        });
        cursor = end;
        textStart = end;
        break;
      }
      parts.push({
        kind: 'interpolation',
        span: { start: cursor, end: expressionEnd + 1 },
        expression: { start: expressionStart, end: expressionEnd },
      });
      cursor = expressionEnd + 1;
      textStart = cursor;
    }
    if (textStart < end)
      parts.push({ kind: 'text', span: { start: textStart, end } });
    const value: StringValue = {
      kind: 'string',
      raw: token.span,
      parts,
      span: token.span,
    };
    return this.addValue(value);
  }

  private parseRegex(): ValueId {
    const token = this.current();
    this.advance();
    let slash = token.span.end - 1;
    while (slash > token.span.start && this.source[slash] !== '/') slash -= 1;
    const value: RegexValue = {
      kind: 'regex',
      pattern: {
        start: token.span.start + 1,
        end: Math.max(token.span.start + 1, slash),
      },
      flags: {
        start: Math.min(slash + 1, token.span.end),
        end: token.span.end,
      },
      span: token.span,
    };
    return this.addValue(value);
  }

  private parseArray(): ArrayValue {
    const open = this.current();
    this.advance();
    const items: ValueId[] = [];
    this.enterDepth(open.span);
    this.skipTriviaAndSeparators();
    while (
      this.current().kind !== TokenKind.RightBracket &&
      this.current().kind !== TokenKind.Eof
    ) {
      if (this.resourceExceeded()) break;
      items.push(this.parseValue());
      if (items.length > this.options.maxCollectionItems) {
        this.error(
          'E0006',
          `collection item limit exceeded (maximum ${this.options.maxCollectionItems})`,
          this.current().span,
        );
        break;
      }
      const separated = this.consumeValueBoundary(TokenKind.RightBracket);
      if (
        !separated &&
        this.current().kind !== TokenKind.RightBracket &&
        this.current().kind !== TokenKind.Eof
      ) {
        this.error(
          'E0203',
          'expected a newline or comma between array values',
          this.current().span,
        );
      }
    }
    const close =
      this.current().kind === TokenKind.RightBracket
        ? this.advance().span
        : this.missingClosing('E0204', 'array', open.span);
    this.leaveDepth();
    return {
      kind: 'array',
      items,
      span: { start: open.span.start, end: close.end },
    };
  }

  private parseObject(explicitRoot: boolean): ObjectValue {
    const open = this.current();
    this.advance();
    this.enterDepth(open.span);
    const members = this.parseMemberList(TokenKind.RightBrace);
    const close =
      this.current().kind === TokenKind.RightBrace
        ? this.advance().span
        : this.missingClosing('E0205', 'object', open.span);
    this.leaveDepth();
    return {
      kind: 'object',
      explicit: explicitRoot,
      members,
      span: { start: open.span.start, end: close.end },
    };
  }

  private parseType(): TypeId {
    const token = this.current();
    if (token.kind !== TokenKind.Atom) {
      this.error('E0301', 'expected a type name', token.span);
      if (token.kind !== TokenKind.Eof) this.advance();
      return this.addType({
        kind: 'error-type',
        code: 'E0301',
        span: token.span,
      });
    }
    const name = this.source.slice(token.span.start, token.span.end);
    this.advance();
    if (name === 'struct' || name === 'enum') {
      const schema = this.parseSchema(name, token.span.start);
      const schemaId = this.addValue(schema);
      return this.addType({
        kind: 'schema',
        schema: schemaId,
        span: { start: token.span.start, end: schema.span.end },
      });
    }
    this.skipTrivia();
    if (this.consume(TokenKind.LessThan)) {
      const argumentsList: TypeId[] = [];
      this.skipTriviaAndSeparators();
      while (
        this.current().kind !== TokenKind.GreaterThan &&
        this.current().kind !== TokenKind.Eof
      ) {
        argumentsList.push(this.parseType());
        this.skipTriviaAndSeparators();
        if (!this.consume(TokenKind.Comma)) break;
        this.skipTriviaAndSeparators();
      }
      const end =
        this.current().kind === TokenKind.GreaterThan
          ? this.advance().span.end
          : this.missingClosing('E0302', 'generic type', token.span).end;
      return this.addType({
        kind: 'generic',
        name: token.span,
        arguments: argumentsList,
        span: { start: token.span.start, end },
      });
    }
    const type: NamedType = {
      kind: 'named',
      name: token.span,
      builtin: builtins.has(name),
      span: token.span,
    };
    return this.addType(type);
  }

  private parseSchema(kind: 'struct' | 'enum', start: number): SchemaValue {
    this.skipTrivia();
    const open = this.current();
    if (open.kind !== TokenKind.LeftBrace) {
      this.error('E0303', `expected '{' after ${kind}`, open.span);
      return {
        kind: 'schema',
        schemaKind: kind,
        fields: [],
        variants: [],
        span: { start, end: open.span.start },
      };
    }
    this.advance();
    this.enterDepth(open.span);
    const fields: StructField[] = [];
    const variants: EnumVariant[] = [];
    this.skipTriviaAndSeparators();
    while (
      this.current().kind !== TokenKind.RightBrace &&
      this.current().kind !== TokenKind.Eof
    ) {
      const annotations = this.parseAnnotations();
      this.skipTrivia();
      const itemStart = this.current().span.start;
      const key = this.parseKey();
      if (key === null) {
        this.recover(TokenKind.RightBrace);
        continue;
      }
      this.skipTrivia();
      if (kind === 'struct') {
        let typeAnnotation: TypeId | null = null;
        let defaultValue: ValueId | null = null;
        if (this.consume(TokenKind.Colon)) {
          this.skipTrivia();
          typeAnnotation = this.parseType();
          this.skipInlineTrivia();
        }
        if (this.consume(TokenKind.Equals)) {
          this.skipTrivia();
          defaultValue = this.parseValue();
        }
        const end =
          defaultValue === null
            ? typeAnnotation === null
              ? key.raw.end
              : this.typeEnd(typeAnnotation)
            : this.valueEnd(defaultValue);
        fields.push({
          annotations,
          key,
          typeAnnotation,
          defaultValue,
          span: { start: itemStart, end },
        });
      } else {
        let payload: TypeId | null = null;
        if (this.consume(TokenKind.Colon)) {
          this.skipTrivia();
          payload = this.parseType();
        }
        const end = payload === null ? key.raw.end : this.typeEnd(payload);
        variants.push({
          annotations,
          name: key,
          payload,
          span: { start: itemStart, end },
        });
      }
      const separated = this.consumeSchemaBoundary(TokenKind.RightBrace);
      if (
        !separated &&
        this.current().kind !== TokenKind.RightBrace &&
        this.current().kind !== TokenKind.Eof
      ) {
        this.error(
          'E0304',
          `expected a newline or comma between ${kind} entries`,
          this.current().span,
        );
      }
    }
    const close =
      this.current().kind === TokenKind.RightBrace
        ? this.advance().span
        : this.missingClosing('E0305', kind, open.span);
    this.leaveDepth();
    return {
      kind: 'schema',
      schemaKind: kind,
      fields,
      variants,
      span: { start, end: close.end },
    };
  }

  private parseAnnotations(): AnnotationId[] {
    const result: AnnotationId[] = [];
    while (this.current().kind === TokenKind.HashBracket) {
      const start = this.advance().span.start;
      this.skipTrivia();
      const nameToken = this.current();
      if (nameToken.kind !== TokenKind.Atom) {
        this.error('E0401', 'expected an annotation name', nameToken.span);
        this.recover(TokenKind.RightBracket);
        continue;
      }
      this.advance();
      const args: AnnotationArgument[] = [];
      this.skipTrivia();
      if (this.consume(TokenKind.Equals)) {
        this.skipTrivia();
        const value = this.parseValue();
        args.push({
          key: null,
          equals: true,
          value,
          span: { start: nameToken.span.end, end: this.valueEnd(value) },
        });
      }
      while (
        this.current().kind !== TokenKind.RightBracket &&
        this.current().kind !== TokenKind.Eof
      ) {
        this.skipTrivia();
        this.consume(TokenKind.Comma);
        this.skipTrivia();
        if (this.current().kind === TokenKind.RightBracket) break;
        const argumentStart = this.current().span.start;
        const candidate = this.current();
        let key: Key | null = null;
        if (
          candidate.kind === TokenKind.Atom &&
          this.peekSignificant(1).kind === TokenKind.Equals
        ) {
          this.advance();
          key = { raw: candidate.span };
          this.skipTrivia();
          this.consume(TokenKind.Equals);
          this.skipTrivia();
        }
        const value = this.parseValue();
        args.push({
          key,
          equals: key !== null,
          value,
          span: { start: argumentStart, end: this.valueEnd(value) },
        });
        if (args.length >= this.options.maxAnnotationArguments) {
          this.error(
            'E0007',
            `annotation argument limit exceeded (maximum ${this.options.maxAnnotationArguments})`,
            this.current().span,
          );
          this.recover(TokenKind.RightBracket);
          break;
        }
      }
      const close =
        this.current().kind === TokenKind.RightBracket
          ? this.advance().span
          : this.missingClosing('E0402', 'annotation', {
              start,
              end: start + 2,
            });
      result.push(
        this.addAnnotation({
          name: nameToken.span,
          arguments: args,
          span: { start, end: close.end },
        }),
      );
      this.skipTrivia();
    }
    return result;
  }

  private consumeMemberBoundary(endKind: TokenKind): boolean {
    return this.consumeBoundary(endKind);
  }

  private consumeValueBoundary(endKind: TokenKind): boolean {
    return this.consumeBoundary(endKind);
  }

  /** Consumes trivia before and after an optional comma without losing newline information. */
  private consumeBoundary(endKind: TokenKind): boolean {
    let separated = this.skipTrivia();
    if (this.consume(TokenKind.Comma) !== null) {
      separated = true;
      this.skipTrivia();
    }
    return separated || this.current().kind === endKind;
  }

  private consumeSchemaBoundary(endKind: TokenKind): boolean {
    return this.consumeValueBoundary(endKind);
  }

  private skipInlineTrivia(): void {
    while (true) {
      const token = this.current();
      if (
        token.kind !== TokenKind.Whitespace &&
        token.kind !== TokenKind.Comment
      )
        return;
      if (token.hasNewline) return;
      this.advance();
    }
  }

  private skipTrivia(): boolean {
    let hadNewline = false;
    while (true) {
      const token = this.current();
      if (
        token.kind !== TokenKind.Whitespace &&
        token.kind !== TokenKind.Newline &&
        token.kind !== TokenKind.Comment
      )
        break;
      hadNewline ||= token.kind === TokenKind.Newline || token.hasNewline;
      this.advance();
    }
    return hadNewline;
  }

  private skipTriviaAndSeparators(): void {
    while (true) {
      const token = this.current();
      if (token.kind === TokenKind.Comma) {
        this.advance();
        continue;
      }
      if (
        token.kind === TokenKind.Whitespace ||
        token.kind === TokenKind.Newline ||
        token.kind === TokenKind.Comment
      ) {
        this.advance();
        continue;
      }
      return;
    }
  }

  private recover(endKind: TokenKind): number {
    const start = this.current().span.start;
    while (
      this.current().kind !== endKind &&
      this.current().kind !== TokenKind.Eof
    ) {
      const token = this.current();
      this.advance();
      if (
        token.kind === TokenKind.Comma ||
        token.kind === TokenKind.Newline ||
        token.hasNewline
      )
        break;
    }
    return this.current().span.start || start;
  }

  private missingClosing(code: string, name: string, open: Span): Span {
    this.error(code, `unterminated ${name}; expected a closing delimiter`, {
      start: this.source.length,
      end: this.source.length,
    });
    return { start: this.source.length, end: this.source.length };
  }

  private enterDepth(span: Span): void {
    this.depth += 1;
    if (this.depth > this.options.maxDepth) {
      this.error(
        'E0008',
        `nesting depth exceeded (maximum ${this.options.maxDepth})`,
        span,
      );
      this.depth = this.options.maxDepth + 1;
    }
  }

  private leaveDepth(): void {
    if (this.depth > 0) this.depth -= 1;
  }

  private resourceExceeded(): boolean {
    if (
      this.depth <= this.options.maxDepth &&
      this.values.length + this.members.length <= this.options.maxTokens
    )
      return false;
    if (!this.resourceReported) {
      this.resourceReported = true;
      this.error(
        'E0009',
        'parser resource limit exceeded',
        this.current().span,
      );
    }
    return true;
  }

  private current(): Token {
    return (
      this.tokens[this.tokenIndex] ?? {
        kind: TokenKind.Eof,
        span: { start: this.source.length, end: this.source.length },
        hasNewline: false,
      }
    );
  }

  private peekSignificant(offset: number): Token {
    let index = this.tokenIndex + offset;
    while (index < this.tokens.length) {
      const token = this.tokens[index];
      if (
        token !== undefined &&
        token.kind !== TokenKind.Whitespace &&
        token.kind !== TokenKind.Newline &&
        token.kind !== TokenKind.Comment
      )
        return token;
      index += 1;
    }
    return this.current();
  }

  private advance(): Token {
    const token = this.current();
    if (this.tokenIndex < this.tokens.length) this.tokenIndex += 1;
    return token;
  }

  private consume(kind: TokenKind): Token | null {
    if (this.current().kind !== kind) return null;
    return this.advance();
  }

  private error(code: string, message: string, span: Span): void {
    this.diagnostics.push({ code, message, severity: 'error', span });
  }

  private addMember(member: Member): MemberId {
    this.members.push(member);
    return this.members.length - 1;
  }

  private addValue(value: Value): ValueId {
    this.values.push(value);
    return this.values.length - 1;
  }

  private addType(type: TypeExpression): TypeId {
    this.types.push(type);
    return this.types.length - 1;
  }

  private addAnnotation(annotation: Annotation): AnnotationId {
    this.annotations.push(annotation);
    return this.annotations.length - 1;
  }

  private valueEnd(value: ValueId): number {
    return this.values[value]?.span.end ?? this.current().span.end;
  }

  private typeEnd(type: TypeId): number {
    return this.types[type]?.span.end ?? this.current().span.end;
  }
}

function isEnumPath(raw: string): boolean {
  const shorthand = raw.startsWith('.');
  const body = shorthand ? raw.slice(1) : raw;
  if (!shorthand && !body.includes('.')) return false;
  return (
    body.length > 0 &&
    body.split('.').every((segment) => enumSegmentPattern.test(segment))
  );
}

function classifyUnknown(raw: string): UnknownShape {
  if (raw.startsWith('@') && raw.includes('/')) return 'package-like';
  if (
    raw.startsWith('./') ||
    raw.startsWith('../') ||
    raw.startsWith('/') ||
    raw.includes('\\')
  )
    return 'path-like';
  if (/^[~^]?\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$/.test(raw))
    return 'version-like';
  if (/^#[0-9A-Fa-f]{3,8}$/.test(raw)) return 'color-like';
  if (identifierPattern.test(raw)) return 'bare-atom';
  return 'other';
}

export function createDocument(
  source: string,
  tokens: readonly Token[],
  diagnostics: readonly Diagnostic[],
  ast: Ast,
): Document {
  return { source, tokens, diagnostics, ast, root: ast.root };
}
