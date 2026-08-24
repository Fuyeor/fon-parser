// fon-parser/src/parser/mod.rs

use alloc::vec::Vec;

use crate::ParseOptions;
use crate::ast::{Ast, CstNodeKind, Document, NodeId, SyntaxTree};
use crate::diagnostic::Diagnostic;
use crate::lexer::LexResult;
use crate::span::Span;
use crate::token::{Token, TokenKind, Trivia};

pub mod document;
pub mod types;
pub mod values;

/// Stateful parser for one in-memory FON source.
pub(crate) struct Parser<'source> {
    pub(crate) source: &'source str,
    pub(crate) tokens: Vec<Token>,
    pub(crate) trivia: Vec<Trivia>,
    pub(crate) position: usize,
    pub(crate) ast: Ast,
    pub(crate) cst: SyntaxTree,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) max_depth: u32,
    pub(crate) depth: u32,
}

impl<'source> Parser<'source> {
    pub(crate) fn new(source: &'source str, lex_result: LexResult, options: ParseOptions) -> Self {
        Self {
            source,
            tokens: lex_result.tokens,
            trivia: lex_result.trivia,
            position: 0,
            ast: Ast::new(crate::ast::Root::ImplicitObject {
                members: Vec::new(),
            }),
            cst: SyntaxTree::default(),
            diagnostics: lex_result.diagnostics,
            max_depth: options.max_depth,
            depth: 0,
        }
    }

    pub(crate) fn current(&self) -> Token {
        self.tokens[self.position.min(self.tokens.len().saturating_sub(1))]
    }

    pub(crate) fn current_kind(&self) -> TokenKind {
        self.current().kind
    }

    pub(crate) fn current_span(&self) -> Span {
        self.current().span
    }

    pub(crate) fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    pub(crate) fn advance(&mut self) -> Token {
        let token = self.current();
        if self.position + 1 < self.tokens.len() {
            self.position += 1;
        }
        token
    }

    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.current_kind() == kind
    }

    pub(crate) fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.at(kind) {
            Some(self.advance())
        } else {
            None
        }
    }

    pub(crate) fn skip_separators(&mut self) {
        while matches!(self.current_kind(), TokenKind::Newline | TokenKind::Comma) {
            self.advance();
        }
    }

    pub(crate) fn expect(&mut self, kind: TokenKind, code: &'static str, message: &str) -> Token {
        if self.at(kind) {
            return self.advance();
        }
        let span = self.current_span();
        self.diagnostics.push(Diagnostic::new(code, message, span));
        Token {
            kind,
            span: Span::new(span.start, span.start),
        }
    }

    pub(crate) fn push_cst(
        &mut self,
        kind: CstNodeKind,
        span: Span,
        children: Vec<NodeId>,
    ) -> NodeId {
        self.cst.push(kind, span, children)
    }

    pub(crate) fn enter_nested(&mut self, span: Span) -> bool {
        self.depth = self.depth.saturating_add(1);
        if self.depth > self.max_depth {
            self.diagnostics.push(Diagnostic::new(
                "E0001",
                "maximum nesting depth exceeded",
                span,
            ));
            self.depth = self.depth.saturating_sub(1);
            return false;
        }
        true
    }

    pub(crate) fn leave_nested(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub(crate) fn recover_to_member_boundary(&mut self) {
        while !matches!(
            self.current_kind(),
            TokenKind::Newline | TokenKind::Comma | TokenKind::RBrace | TokenKind::Eof
        ) {
            self.advance();
        }
    }

    pub(crate) fn finish(self) -> (Document, Vec<Diagnostic>) {
        let document = Document {
            source: self.source.into(),
            cst: SyntaxTree {
                nodes: self.cst.nodes,
                tokens: self.tokens,
                trivia: self.trivia,
            },
            ast: self.ast,
        };
        (document, self.diagnostics)
    }
}
