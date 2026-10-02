//! Go definitions (tree-sitter-go node kinds). Methods are declared beside their type, so they stay
//! at the top of the outline, their signature naming the receiver.

use tree_sitter::Node;

use super::{Kind, Scope, Visit, define, signature, signature_before};

pub(super) fn classify<'t>(node: Node<'t>, source: &[u8], _scope: Scope) -> Visit<'t> {
  match node.kind() {
    "type_declaration" | "const_declaration" => Visit::Descend,
    "function_declaration" => define(
      node,
      source,
      Kind::Function,
      signature_before(node, source, "body"),
      None,
    ),
    "method_declaration" => define(node, source, Kind::Method, signature_before(node, source, "body"), None),
    "type_spec" | "type_alias" => type_spec(node, source),
    "const_spec" => define(
      node,
      source,
      Kind::Const,
      format!("const {}", signature_before(node, source, "value")),
      None,
    ),
    _ => Visit::Skip,
  }
}

/// `type Name struct {…}`: a struct, an interface or another type, declared up to its fields.
fn type_spec<'t>(node: Node<'t>, source: &[u8]) -> Visit<'t> {
  let definition = node.child_by_field_name("type");
  let kind = match definition.map(|definition| definition.kind()) {
    Some("struct_type") => Kind::Struct,
    Some("interface_type") => Kind::Interface,
    _ => Kind::Type,
  };
  let text = &source[node.byte_range()];
  let end = text
    .iter()
    .position(|byte| *byte == b'{')
    .map_or(node.end_byte(), |offset| node.start_byte() + offset);
  define(
    node,
    source,
    kind,
    format!("type {}", signature(node, source, end)),
    None,
  )
}
