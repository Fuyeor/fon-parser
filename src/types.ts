// src/types.ts

/** A half-open UTF-16 source range used by every syntax node. */
export interface Span {
  readonly start: number;
  readonly end: number;
}

/** Numeric token kinds keep token storage compact while remaining inspectable. */
export const TokenKind = {
  Eof: 0,
  Whitespace: 1,
  Newline: 2,
  Comment: 3,
  Atom: 4,
  String: 5,
  Regex: 6,
  HashBracket: 7,
  LeftBrace: 8,
  RightBrace: 9,
  LeftBracket: 10,
  RightBracket: 11,
  LeftParen: 12,
  RightParen: 13,
  Comma: 14,
  Colon: 15,
  Equals: 16,
  LessThan: 17,
  GreaterThan: 18,
} as const;

export type TokenKind = (typeof TokenKind)[keyof typeof TokenKind];

export interface Token {
  readonly kind: TokenKind;
  readonly span: Span;
  /** True when trivia contains a physical line break. */
  readonly hasNewline: boolean;
}

export type DiagnosticSeverity = "error" | "warning";

export interface Diagnostic {
  readonly code: string;
  readonly message: string;
  readonly severity: DiagnosticSeverity;
  readonly span: Span;
}

export interface ParseOptions {
  readonly maxDepth: number;
  readonly maxTokens: number;
  readonly maxTokenLength: number;
  readonly maxSourceLength: number;
  readonly maxCollectionItems: number;
  readonly maxAnnotationArguments: number;
}

export const defaultParseOptions: ParseOptions = Object.freeze({
  maxDepth: 256,
  maxTokens: 1_000_000,
  maxTokenLength: 1_048_576,
  maxSourceLength: 64 * 1024 * 1024,
  maxCollectionItems: 1_000_000,
  maxAnnotationArguments: 65_536,
});

export type NodeId = number;
export type MemberId = number;
export type ValueId = number;
export type TypeId = number;
export type AnnotationId = number;

export interface Key {
  readonly raw: Span;
}

export interface RootBase {
  readonly annotations: readonly AnnotationId[];
  readonly span: Span;
}

export interface ImplicitObjectRoot extends RootBase {
  readonly kind: "implicit-object";
  readonly members: readonly MemberId[];
}

export interface ExplicitObjectRoot extends RootBase {
  readonly kind: "explicit-object";
  readonly members: readonly MemberId[];
}

export interface ArrayRoot extends RootBase {
  readonly kind: "array";
  readonly items: readonly ValueId[];
}

export type Root = ImplicitObjectRoot | ExplicitObjectRoot | ArrayRoot;

export interface BindingMember {
  readonly kind: "binding";
  readonly annotations: readonly AnnotationId[];
  readonly key: Key;
  readonly typeAnnotation: TypeId | null;
  readonly value: ValueId;
  readonly span: Span;
}

export interface TypeDeclarationMember {
  readonly kind: "type-declaration";
  readonly annotations: readonly AnnotationId[];
  readonly key: Key;
  readonly schema: TypeId;
  readonly span: Span;
}

export interface ErrorMember {
  readonly kind: "error-member";
  readonly annotations: readonly AnnotationId[];
  readonly span: Span;
}

export type Member = BindingMember | TypeDeclarationMember | ErrorMember;

export interface BooleanValue {
  readonly kind: "boolean";
  readonly value: boolean;
  readonly span: Span;
}

export interface NumberValue {
  readonly kind: "number";
  readonly raw: Span;
  readonly integer: boolean;
  readonly span: Span;
}

export interface StringTextPart {
  readonly kind: "text";
  readonly span: Span;
}

export interface StringInterpolationPart {
  readonly kind: "interpolation";
  readonly span: Span;
  readonly expression: Span;
}

export type StringPart = StringTextPart | StringInterpolationPart;

export interface StringValue {
  readonly kind: "string";
  readonly raw: Span;
  readonly parts: readonly StringPart[];
  readonly span: Span;
}

export interface RegexValue {
  readonly kind: "regex";
  readonly pattern: Span;
  readonly flags: Span;
  readonly span: Span;
}

export interface EnumPathValue {
  readonly kind: "enum-path";
  readonly shorthand: boolean;
  readonly segments: readonly Span[];
  readonly span: Span;
}

export interface ArrayValue {
  readonly kind: "array";
  readonly items: readonly ValueId[];
  readonly span: Span;
}

export interface ObjectValue {
  readonly kind: "object";
  readonly explicit: boolean;
  readonly members: readonly MemberId[];
  readonly span: Span;
}

export type SchemaKind = "struct" | "enum";

export interface StructField {
  readonly annotations: readonly AnnotationId[];
  readonly key: Key;
  readonly typeAnnotation: TypeId | null;
  readonly defaultValue: ValueId | null;
  readonly span: Span;
}

export interface EnumVariant {
  readonly annotations: readonly AnnotationId[];
  readonly name: Key;
  readonly payload: TypeId | null;
  readonly span: Span;
}

export interface SchemaValue {
  readonly kind: "schema";
  readonly schemaKind: SchemaKind;
  readonly fields: readonly StructField[];
  readonly variants: readonly EnumVariant[];
  readonly span: Span;
}

export type UnknownShape =
  | "bare-atom"
  | "package-like"
  | "path-like"
  | "version-like"
  | "color-like"
  | "other";

export interface UnknownValue {
  readonly kind: "unknown";
  readonly raw: Span;
  readonly shape: UnknownShape;
  readonly span: Span;
}

export interface ErrorValue {
  readonly kind: "error";
  readonly code: string;
  readonly span: Span;
}

export type Value =
  | BooleanValue
  | NumberValue
  | StringValue
  | RegexValue
  | EnumPathValue
  | ArrayValue
  | ObjectValue
  | SchemaValue
  | UnknownValue
  | ErrorValue;

export interface NamedType {
  readonly kind: "named";
  readonly name: Span;
  readonly builtin: boolean;
  readonly span: Span;
}

export interface GenericType {
  readonly kind: "generic";
  readonly name: Span;
  readonly arguments: readonly TypeId[];
  readonly span: Span;
}

export interface SchemaType {
  readonly kind: "schema";
  readonly schema: ValueId;
  readonly span: Span;
}

export interface ErrorType {
  readonly kind: "error-type";
  readonly code: string;
  readonly span: Span;
}

export type TypeExpression = NamedType | GenericType | SchemaType | ErrorType;

export interface AnnotationArgument {
  readonly key: Key | null;
  readonly equals: boolean;
  readonly value: ValueId;
  readonly span: Span;
}

export interface Annotation {
  readonly name: Span;
  readonly arguments: readonly AnnotationArgument[];
  readonly span: Span;
}

export interface Ast {
  readonly root: Root;
  readonly members: readonly Member[];
  readonly values: readonly Value[];
  readonly types: readonly TypeExpression[];
  readonly annotations: readonly Annotation[];
}

export interface Document {
  readonly source: string;
  readonly tokens: readonly Token[];
  readonly diagnostics: readonly Diagnostic[];
  readonly ast: Ast;
  readonly root: Root;
}

export class ParseResult {
  public readonly document: Document;

  public constructor(document: Document) {
    this.document = document;
  }

  public get diagnostics(): readonly Diagnostic[] {
    return this.document.diagnostics;
  }

  public hasErrors(): boolean {
    return this.document.diagnostics.some((diagnostic) => diagnostic.severity === "error");
  }
}

export function text(document: Document, span: Span): string {
  return document.source.slice(span.start, span.end);
}

export interface Visitor {
  visitDocument?(document: Document): void;
  visitMember?(member: Member, id: MemberId): void;
  visitValue?(value: Value, id: ValueId): void;
  visitType?(type: TypeExpression, id: TypeId): void;
  visitAnnotation?(annotation: Annotation, id: AnnotationId): void;
}
