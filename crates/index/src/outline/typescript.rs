//! TypeScript and JavaScript definitions (tree-sitter-typescript node kinds, shared by its
//! TypeScript and TSX grammars).

use tree_sitter::Node;

use super::{Kind, Scope, Visit, define, signature, signature_before};

pub(super) fn classify<'t>(node: Node<'t>, source: &[u8], scope: Scope) -> Visit<'t> {
  let body = node.child_by_field_name("body");
  let before_body = || signature_before(node, source, "body");
  match node.kind() {
    "export_statement" | "ambient_declaration" | "lexical_declaration" | "variable_declaration" => Visit::Descend,
    // `namespace Name {…}` is an expression statement to the grammar.
    "expression_statement" if scope == Scope::Top || scope == Scope::In(Kind::Namespace) => Visit::Descend,
    "function_declaration" | "generator_function_declaration" | "function_signature" => {
      define(node, source, Kind::Function, before_body(), None)
    }
    "class_declaration" | "abstract_class_declaration" => define(node, source, Kind::Class, before_body(), body),
    "interface_declaration" => define(node, source, Kind::Interface, before_body(), body),
    "internal_module" | "module" => define(node, source, Kind::Namespace, before_body(), body),
    "enum_declaration" => define(node, source, Kind::Enum, before_body(), None),
    "type_alias_declaration" => define(node, source, Kind::Type, signature_before(node, source, "value"), None),
    "method_definition" | "method_signature" | "abstract_method_signature" => member(node, source, scope),
    "variable_declarator" if scope == Scope::Top || scope == Scope::In(Kind::Namespace) => variable(node, source),
    _ => Visit::Skip,
  }
}

/// A method of a class or interface.
fn member<'t>(node: Node<'t>, source: &[u8], scope: Scope) -> Visit<'t> {
  if !matches!(scope, Scope::In(Kind::Class | Kind::Interface)) {
    return Visit::Skip;
  }
  define(node, source, Kind::Method, signature_before(node, source, "body"), None)
}

/// `const name = …` (or `let`, `var`) at the top of a module or namespace: a function when its
/// value is one, a constant otherwise.
fn variable<'t>(node: Node<'t>, source: &[u8]) -> Visit<'t> {
  let Some(name) = node
    .child_by_field_name("name")
    .filter(|name| name.kind() == "identifier")
  else {
    return Visit::Skip;
  };
  let keyword = node
    .parent()
    .and_then(|declaration| declaration.child(0))
    .map_or("", |keyword| keyword.kind());
  let value = node.child_by_field_name("value");
  let function = value.filter(|value| {
    matches!(
      value.kind(),
      "arrow_function" | "function_expression" | "function" | "generator_function"
    )
  });
  let (kind, end) = match function {
    Some(function) => (
      Kind::Function,
      function
        .child_by_field_name("body")
        .map_or(function.end_byte(), |body| body.start_byte()),
    ),
    None => (Kind::Const, value.map_or(node.end_byte(), |value| value.start_byte())),
  };
  let declared = signature(node, source, end);
  let declared = declared.trim_end_matches("=>").trim_end();
  Visit::Define {
    kind,
    name: super::text(name, source),
    signature: format!("{keyword} {declared}").trim_start().to_owned(),
    body: None,
  }
}
