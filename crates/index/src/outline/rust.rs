//! Rust definitions (tree-sitter-rust node kinds).

use tree_sitter::Node;

use super::{Kind, Scope, Visit, define, signature, signature_before, text};

pub(super) fn classify<'t>(node: Node<'t>, source: &[u8], scope: Scope) -> Visit<'t> {
  let body = node.child_by_field_name("body");
  let before_body = || signature_before(node, source, "body");
  match node.kind() {
    "function_item" | "function_signature_item" => define(node, source, function_kind(scope), before_body(), None),
    "struct_item" => define(node, source, Kind::Struct, before_body(), None),
    "enum_item" => define(node, source, Kind::Enum, before_body(), None),
    "union_item" => define(node, source, Kind::Union, before_body(), None),
    "trait_item" => define(node, source, Kind::Trait, before_body(), body),
    "mod_item" => define(node, source, Kind::Module, before_body(), body),
    "const_item" => define(node, source, Kind::Const, signature_before(node, source, "value"), None),
    "static_item" => define(
      node,
      source,
      Kind::Static,
      signature_before(node, source, "value"),
      None,
    ),
    "type_item" | "associated_type" => define(node, source, Kind::Type, signature_before(node, source, "type"), None),
    "macro_definition" => {
      let end = node
        .child_by_field_name("name")
        .map_or(node.end_byte(), |name| name.end_byte());
      define(node, source, Kind::Macro, signature(node, source, end), None)
    }
    "impl_item" => impl_item(node, source, body),
    _ => Visit::Skip,
  }
}

const fn function_kind(scope: Scope) -> Kind {
  match scope {
    Scope::In(Kind::Impl | Kind::Trait) => Kind::Method,
    _ => Kind::Function,
  }
}

/// An impl is named for its type, and for its trait when it implements one:
/// `Display for Worktree`.
fn impl_item<'t>(node: Node<'t>, source: &[u8], body: Option<Node<'t>>) -> Visit<'t> {
  let Some(target) = node.child_by_field_name("type") else {
    return Visit::Skip;
  };
  let name = match node.child_by_field_name("trait") {
    Some(implemented) => format!("{} for {}", text(implemented, source), text(target, source)),
    None => text(target, source),
  };
  Visit::Define {
    kind: Kind::Impl,
    name,
    signature: signature_before(node, source, "body"),
    body,
  }
}
