// fon-parser/tests/cst.rs

use fon_parser::cst::Visitor;
use fon_parser::{CstNodeKind, parse};

struct NodeCounter {
    count: usize,
}

impl Visitor for NodeCounter {
    fn visit_node(&mut self, _node: &fon_parser::CstNode) {
        self.count += 1;
    }
}

#[test]
fn preserves_object_to_member_cst_children() {
    let result = parse("{ first = 1\nsecond = { nested = true } }");

    assert!(
        !result.has_errors(),
        "unexpected diagnostics: {:?}",
        result.diagnostics
    );
    let root = result
        .document
        .cst
        .nodes
        .iter()
        .find(|node| node.kind == CstNodeKind::Object && node.span.start == 0)
        .expect("root object node");
    assert_eq!(root.children.len(), 2);
    assert!(
        root.children
            .iter()
            .all(|child| result.document.cst.nodes[child.0 as usize].kind == CstNodeKind::Binding)
    );
}

#[test]
fn visitor_walks_the_indexed_cst() {
    let result = parse("name = `Fuyeor`\n");
    let mut counter = NodeCounter { count: 0 };

    counter.visit_tree(&result.document.cst);

    assert!(counter.count >= 2);
}
