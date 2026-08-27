// src/lexer.ts

import {
  defaultParseOptions,
  Diagnostic,
  ParseOptions,
  Span,
  Token,
  TokenKind,
} from "./types.js";

export interface LexResult {
  readonly tokens: Token[];
  readonly diagnostics: Diagnostic[];
}

const punctuationKinds: Readonly<Record<string, TokenKind>> = {
  "{": TokenKind.LeftBrace,
  "}": TokenKind.RightBrace,
  "[": TokenKind.LeftBracket,
  "]": TokenKind.RightBracket,
  "(": TokenKind.LeftParen,
  ")": TokenKind.RightParen,
  ",": TokenKind.Comma,
  ":": TokenKind.Colon,
  "=": TokenKind.Equals,
  "<": TokenKind.LessThan,
  ">": TokenKind.GreaterThan,
};

function isAsciiLetter(character: string): boolean {
  const code = character.charCodeAt(0);
  return (code >= 65 && code <= 90) || (code >= 97 && code <= 122);
}

/** Scans source once and keeps every physical token needed for lossless tooling. */
export function lex(source: string, options: ParseOptions = defaultParseOptions): LexResult {
  const tokens: Token[] = [];
  const diagnostics: Diagnostic[] = [];
  let index = 0;
  let limitReported = false;

  const addDiagnostic = (code: string, message: string, start: number, end = start + 1): void => {
    diagnostics.push({ code, message, severity: "error", span: { start, end } });
  };

  const push = (kind: TokenKind, start: number, end: number, hasNewline = false): boolean => {
    if (tokens.length >= options.maxTokens) {
      if (!limitReported) {
        limitReported = true;
        addDiagnostic("E0001", `token limit exceeded (maximum ${options.maxTokens})`, start, end);
      }
      return false;
    }
    tokens.push({ kind, span: { start, end }, hasNewline });
    return true;
  };

  const isAtomBoundary = (character: string): boolean =>
    character === " " || character === "\t" || character === "\v" || character === "\f" ||
    character === "\r" || character === "\n" || character === "," || character === "{" ||
    character === "}" || character === "[" || character === "]" || character === "(" ||
    character === ")" || character === ":" || character === "=" || character === "`" ||
    character === "<" || character === ">";

  const scanDelimitedString = (start: number): number => {
    let cursor = start + 1;
    while (cursor < source.length) {
      const character = source[cursor];
      if (character === "\\") {
        cursor += 2;
        continue;
      }
      if (character === "`") return cursor + 1;
      cursor += 1;
    }
    return source.length;
  };

  const scanRegex = (start: number): number => {
    let cursor = start + 1;
    let inCharacterClass = false;
    while (cursor < source.length) {
      const character = source[cursor];
      if (character === "\\") {
        cursor += 2;
        continue;
      }
      if (character === "[") inCharacterClass = true;
      else if (character === "]") inCharacterClass = false;
      else if (character === "/" && !inCharacterClass) {
        cursor += 1;
        while (cursor < source.length && isAsciiLetter(source[cursor] ?? "")) cursor += 1;
        return cursor;
      }
      if (character === "\n" || character === "\r") return -1;
      cursor += 1;
    }
    return -1;
  };

  const scanBlockComment = (start: number): number => {
    const close = source.indexOf("*/", start + 2);
    return close < 0 ? source.length : close + 2;
  };

  while (index < source.length) {
    const start = index;
    const character = source[index] ?? "";

    if (character === " " || character === "\t" || character === "\v" || character === "\f") {
      index += 1;
      while (index < source.length) {
        const next = source[index];
        if (next !== " " && next !== "\t" && next !== "\v" && next !== "\f") break;
        index += 1;
      }
      if (!push(TokenKind.Whitespace, start, index)) break;
      continue;
    }

    if (character === "\r" || character === "\n") {
      index += character === "\r" && source[index + 1] === "\n" ? 2 : 1;
      if (!push(TokenKind.Newline, start, index, true)) break;
      continue;
    }

    if (character === "/" && source[index + 1] === "/") {
      index += 2;
      while (index < source.length && source[index] !== "\r" && source[index] !== "\n") index += 1;
      if (!push(TokenKind.Comment, start, index, false)) break;
      continue;
    }

    if (character === "/" && source[index + 1] === "*") {
      index = scanBlockComment(start);
      const closed = source.slice(index - 2, index) === "*/";
      const hasNewline = /\r|\n/.test(source.slice(start, index));
      if (!push(TokenKind.Comment, start, index, hasNewline)) break;
      if (!closed) addDiagnostic("E0003", "unterminated block comment", start, index);
      continue;
    }

    if (character === "`") {
      index = scanDelimitedString(start);
      if (index - start > options.maxTokenLength) {
        addDiagnostic("E0002", `token length exceeded (maximum ${options.maxTokenLength})`, start, index);
      }
      if (!push(TokenKind.String, start, index)) break;
      if (source[index - 1] !== "`") addDiagnostic("E0004", "unterminated backtick string", start, index);
      continue;
    }

    if (character === "/" && source[index + 1] !== "/" && source[index + 1] !== "*") {
      const regexEnd = scanRegex(start);
      if (regexEnd > start) {
        index = regexEnd;
        if (index - start > options.maxTokenLength) {
          addDiagnostic("E0002", `token length exceeded (maximum ${options.maxTokenLength})`, start, index);
        }
        if (!push(TokenKind.Regex, start, index)) break;
        continue;
      }
    }

    if (character === "#" && source[index + 1] === "[") {
      index += 2;
      if (!push(TokenKind.HashBracket, start, index)) break;
      continue;
    }
    const punctuationKind = punctuationKinds[character];
    if (punctuationKind !== undefined) {
      index += 1;
      if (!push(punctuationKind, start, index)) break;
      continue;
    }

    index += 1;
    while (index < source.length && !isAtomBoundary(source[index] ?? "")) {
      if (source[index] === "/" && source[index + 1] === "/") break;
      if (source[index] === "/" && source[index + 1] === "*") break;
      index += 1;
    }
    if (index - start > options.maxTokenLength) {
      addDiagnostic("E0002", `token length exceeded (maximum ${options.maxTokenLength})`, start, index);
    }
    if (!push(TokenKind.Atom, start, index)) break;
  }

  if (source.length > options.maxSourceLength) {
    addDiagnostic("E0005", `source length exceeded (maximum ${options.maxSourceLength})`, options.maxSourceLength, source.length);
  }
  push(TokenKind.Eof, source.length, source.length);
  return { tokens, diagnostics };
}
