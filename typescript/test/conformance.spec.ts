// typescript/test/conformance.spec.ts

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parse, reprintLossless, text } from '../src/index.js';
import type {
  Annotation,
  Document,
  Member,
  Root,
  SchemaValue,
  Span,
  TypeExpression,
  Value,
} from '../src/index.js';

interface FixtureCase {
  readonly id: string;
  readonly kind: 'valid' | 'invalid' | 'limit';
  readonly path: string;
  readonly errorCategory?: string;
  readonly limits?: { readonly maxDepth?: number };
}

interface Manifest {
  readonly schemaVersion: number;
  readonly profile: string;
  readonly cases: readonly FixtureCase[];
}

interface ExpectedFixture {
  readonly schemaVersion: number;
  readonly status: 'pass' | 'error';
  readonly projection?: ProjectedDocument;
  readonly errorCategory?: string;
  readonly lossless?: boolean;
}

interface ProjectedDocument {
  readonly root: ProjectedRoot;
}

interface ProjectedRoot {
  readonly kind: Root['kind'];
  readonly annotations: readonly ProjectedAnnotation[];
  readonly members?: readonly ProjectedMember[];
  readonly items?: readonly ProjectedValue[];
}

interface ProjectedMember {
  readonly kind: Member['kind'];
  readonly key?: string;
  readonly annotations: readonly ProjectedAnnotation[];
  readonly type?: string;
  readonly value?: ProjectedValue;
  readonly schema?: ProjectedSchema;
}

interface ProjectedAnnotation {
  readonly name: string;
  readonly arguments: readonly {
    readonly key: string | null;
    readonly value: ProjectedValue | null;
  }[];
}

interface ProjectedSchema {
  readonly schemaKind: SchemaValue['schemaKind'];
  readonly fields: readonly {
    readonly key: string;
    readonly type: string | null;
    readonly default: ProjectedValue | null;
  }[];
  readonly variants: readonly {
    readonly key: string;
    readonly type: string | null;
  }[];
}

interface ProjectedValue {
  readonly kind: Value['kind'];
  readonly value?: boolean;
  readonly raw?: string;
  readonly integer?: boolean;
  readonly parts?: readonly {
    readonly kind: 'text' | 'interpolation';
    readonly text?: string;
    readonly expression?: string;
  }[];
  readonly pattern?: string;
  readonly flags?: string;
  readonly shorthand?: boolean;
  readonly path?: string;
  readonly items?: readonly ProjectedValue[];
  readonly members?: readonly ProjectedMember[];
  readonly schema?: ProjectedSchema;
  readonly shape?: string;
}

const manifestUrl = new URL(
  '../../fixtures/fon-core/manifest.json',
  import.meta.url,
);
const fixtureRootUrl = new URL('../../fixtures/fon-core/', import.meta.url);
const manifest = JSON.parse(readFileSync(manifestUrl, 'utf8')) as Manifest;

describe('FON Core cross-language fixtures', () => {
  for (const fixture of manifest.cases) {
    it(`${fixture.kind}: ${fixture.id}`, () => {
      const caseUrl = new URL(`${fixture.path}/`, fixtureRootUrl);
      const source = readFileSync(new URL('input.fon', caseUrl), 'utf8');
      const expected = JSON.parse(
        readFileSync(new URL('expected.json', caseUrl), 'utf8'),
      ) as ExpectedFixture;
      const result = parse(source, fixture.limits ?? {});
      expect(expected.schemaVersion).toBe(manifest.schemaVersion);
      if (fixture.kind === 'valid') {
        expect(result.hasErrors(), result.diagnostics).toBe(false);
        expect(projectDocument(result.document)).toEqual(expected.projection);
        if (expected.lossless === true)
          expect(reprintLossless(result.document)).toBe(source);
        return;
      }
      expect(result.hasErrors()).toBe(true);
      expect(diagnosticCategory(result.diagnostics)).toBe(
        fixture.errorCategory,
      );
      expect(diagnosticCategory(result.diagnostics)).toBe(
        expected.errorCategory,
      );
    });
  }
});

/** Converts implementation-specific AST storage into the shared fixture projection. */
function projectDocument(document: Document): ProjectedDocument {
  return { root: projectRoot(document, document.root) };
}

function projectRoot(document: Document, root: Root): ProjectedRoot {
  const annotations = root.annotations.map((id) =>
    projectAnnotation(document, id),
  );
  if (root.kind === 'array') {
    return {
      kind: root.kind,
      annotations,
      items: root.items.map((id) => projectValue(document, id)),
    };
  }
  return {
    kind: root.kind,
    annotations,
    members: root.members.map((id) => projectMember(document, id)),
  };
}

function projectMember(document: Document, memberId: number): ProjectedMember {
  const member = document.ast.members[memberId];
  if (member === undefined) throw new Error(`missing member ${memberId}`);
  const annotations = member.annotations.map((id) =>
    projectAnnotation(document, id),
  );
  if (member.kind === 'error-member') return { kind: member.kind, annotations };
  const key = text(document, member.key.raw);
  if (member.kind === 'type-declaration') {
    const schemaValue =
      document.ast.values[
        document.ast.types[member.schema]?.kind === 'schema'
          ? document.ast.types[member.schema].schema
          : -1
      ];
    if (schemaValue?.kind !== 'schema')
      throw new Error(`missing schema for ${key}`);
    return {
      kind: member.kind,
      key,
      annotations,
      schema: projectSchema(document, schemaValue),
    };
  }
  return {
    kind: member.kind,
    key,
    annotations,
    ...(member.typeAnnotation === null
      ? {}
      : { type: projectType(document, member.typeAnnotation) }),
    value: projectValue(document, member.value),
  };
}

function projectValue(document: Document, valueId: number): ProjectedValue {
  const value = document.ast.values[valueId];
  if (value === undefined) throw new Error(`missing value ${valueId}`);
  return projectValueNode(document, value);
}

function projectValueNode(document: Document, value: Value): ProjectedValue {
  switch (value.kind) {
    case 'boolean':
      return { kind: value.kind, value: value.value };
    case 'number': {
      const raw = text(document, value.raw);
      return { kind: value.kind, raw, integer: value.integer };
    }
    case 'string':
      return {
        kind: value.kind,
        raw: text(document, value.raw),
        parts: value.parts.map((part) =>
          part.kind === 'text'
            ? { kind: part.kind, text: text(document, part.span) }
            : { kind: part.kind, expression: text(document, part.expression) },
        ),
      };
    case 'regex':
      return {
        kind: value.kind,
        pattern: text(document, value.pattern),
        flags: text(document, value.flags),
      };
    case 'enum-path':
      return {
        kind: value.kind,
        shorthand: value.shorthand,
        path: text(document, value.span),
      };
    case 'array':
      return {
        kind: value.kind,
        items: value.items.map((id) => projectValue(document, id)),
      };
    case 'object':
      return {
        kind: value.kind,
        members: value.members.map((id) => projectMember(document, id)),
      };
    case 'schema':
      return { kind: value.kind, schema: projectSchema(document, value) };
    case 'unknown':
      return {
        kind: value.kind,
        raw: text(document, value.raw),
        shape: value.shape,
      };
    case 'error':
      return { kind: value.kind };
  }
}

function projectSchema(
  document: Document,
  schema: SchemaValue,
): ProjectedSchema {
  return {
    schemaKind: schema.schemaKind,
    fields: schema.fields.map((field) => ({
      key: text(document, field.key.raw),
      type:
        field.typeAnnotation === null
          ? null
          : projectType(document, field.typeAnnotation),
      default:
        field.defaultValue === null
          ? null
          : projectValue(document, field.defaultValue),
    })),
    variants: schema.variants.map((variant) => ({
      key: text(document, variant.name.raw),
      type:
        variant.payload === null
          ? null
          : projectType(document, variant.payload),
    })),
  };
}

function projectType(document: Document, typeId: number): string {
  const type = document.ast.types[typeId];
  if (type === undefined) throw new Error(`missing type ${typeId}`);
  return projectTypeNode(document, type);
}

function projectTypeNode(document: Document, type: TypeExpression): string {
  return text(document, type.span);
}

function projectAnnotation(
  document: Document,
  annotationId: number,
): ProjectedAnnotation {
  const annotation = document.ast.annotations[annotationId];
  if (annotation === undefined)
    throw new Error(`missing annotation ${annotationId}`);
  return {
    name: text(document, annotation.name),
    arguments: annotation.arguments.map((argument) => ({
      key: argument.key === null ? null : text(document, argument.key.raw),
      value: projectValue(document, argument.value),
    })),
  };
}

function diagnosticCategory(
  diagnostics: readonly { readonly message: string }[],
): string {
  const message = diagnostics
    .map((diagnostic) => diagnostic.message)
    .join(' ')
    .toLowerCase();
  if (/depth|nesting|token limit|resource/.test(message))
    return 'resource-limit';
  if (/expected .*value|missing value/.test(message)) return 'missing-value';
  if (/closing|unterminated/.test(message)) return 'unclosed-delimiter';
  if (/newline|comma|separator|between .*member|between .*value/.test(message))
    return 'missing-separator';
  return 'syntax-error';
}
