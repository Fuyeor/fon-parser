// fon-parser/src/cst.rs

pub use crate::ast::{CstNode, CstNodeKind, NodeId, SyntaxTree};

/// A read-only visitor over the indexed concrete syntax tree.
pub trait Visitor {
    fn visit_node(&mut self, _node: &CstNode) {}

    fn visit_tree(&mut self, tree: &SyntaxTree) {
        for node in &tree.nodes {
            self.visit_node(node);
        }
    }
}
