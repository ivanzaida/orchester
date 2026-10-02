//! Python definitions (tree-sitter-python node kinds).

use tree_sitter::Node;

use super::{Kind, Scope, Visit, define, signature_before};

pub(super) fn classify<'t>(node: Node<'t>, source: &[u8], scope: Scope) -> Visit<'t> {
  match node.kind() {
    "decorated_definition" => Visit::Descend,
    "class_definition" => define(
      node,
      source,
      Kind::Class,
      signature_before(node, source, "body"),
      node.child_by_field_name("body"),
    ),
    "function_definition" => {
      let kind = if scope == Scope::In(Kind::Class) {
        Kind::Method
      } else {
        Kind::Function
      };
      define(node, source, kind, signature_before(node, source, "body"), None)
    }
    _ => Visit::Skip,
  }
}
