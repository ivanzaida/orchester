//! The definitions of a source file, read from its tree-sitter syntax tree: types, traits,
//! functions, methods, impls, modules, classes and constants, nested where the language nests
//! them (impl → methods, class → methods, mod → items). Function bodies are not searched: what is
//! defined inside one is not part of the file's outline.

mod go;
mod python;
mod rust;
mod typescript;

use tree_sitter::{Node, Parser, Tree};

use crate::language::Language;

/// The longest signature kept, in characters.
const SIGNATURE_LIMIT: usize = 200;

/// What a definition is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
  Module,
  Struct,
  Enum,
  Union,
  Trait,
  Impl,
  Function,
  Method,
  Const,
  Static,
  Type,
  Macro,
  Class,
  Interface,
  Namespace,
}

impl Kind {
  const ALL: [Self; 15] = [
    Self::Module,
    Self::Struct,
    Self::Enum,
    Self::Union,
    Self::Trait,
    Self::Impl,
    Self::Function,
    Self::Method,
    Self::Const,
    Self::Static,
    Self::Type,
    Self::Macro,
    Self::Class,
    Self::Interface,
    Self::Namespace,
  ];

  pub(crate) const fn name(self) -> &'static str {
    match self {
      Self::Module => "module",
      Self::Struct => "struct",
      Self::Enum => "enum",
      Self::Union => "union",
      Self::Trait => "trait",
      Self::Impl => "impl",
      Self::Function => "function",
      Self::Method => "method",
      Self::Const => "const",
      Self::Static => "static",
      Self::Type => "type",
      Self::Macro => "macro",
      Self::Class => "class",
      Self::Interface => "interface",
      Self::Namespace => "namespace",
    }
  }

  pub(crate) fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|kind| kind.name() == name)
  }
}

/// A definition with the lines it spans and what it holds.
#[derive(Debug)]
pub(crate) struct Item {
  pub(crate) kind: Kind,
  pub(crate) name: String,
  /// Its declaration up to its body, on one line.
  pub(crate) signature: String,
  pub(crate) start: usize,
  pub(crate) end: usize,
  pub(crate) children: Vec<Self>,
}

/// What a node of the syntax tree is to the outline.
enum Visit<'t> {
  /// A definition; `body`, when set, holds its nested definitions.
  Define {
    kind: Kind,
    name: String,
    signature: String,
    body: Option<Node<'t>>,
  },
  /// A wrapper (an export, a decorator) whose children may be definitions.
  Descend,
  Skip,
}

/// Where a node is: at the top of the file or a namespace, or inside a definition of `Kind`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
  Top,
  In(Kind),
}

type Classify = for<'t> fn(Node<'t>, &[u8], Scope) -> Visit<'t>;

/// The definitions in `source`, a file of `language`; `None` when the language has no grammar or
/// tree-sitter gives no tree.
pub(crate) fn items(language: Language, source: &[u8]) -> Option<Vec<Item>> {
  let (grammar, classify): (tree_sitter::Language, Classify) = match language {
    Language::Rust => (tree_sitter_rust::LANGUAGE.into(), rust::classify),
    Language::TypeScript => (tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), typescript::classify),
    // The TSX grammar reads JavaScript with JSX as well; the TypeScript one cannot, because of
    // its `<T>value` casts.
    Language::Tsx | Language::JavaScript => (tree_sitter_typescript::LANGUAGE_TSX.into(), typescript::classify),
    Language::Python => (tree_sitter_python::LANGUAGE.into(), python::classify),
    Language::Go => (tree_sitter_go::LANGUAGE.into(), go::classify),
    Language::Markdown | Language::Plain(_) => return None,
  };
  let tree = parse(&grammar, source)?;
  let mut items = Vec::new();
  collect(tree.root_node(), source, classify, Scope::Top, &mut items);
  Some(items)
}

fn parse(grammar: &tree_sitter::Language, source: &[u8]) -> Option<Tree> {
  let mut parser = Parser::new();
  parser
    .set_language(grammar)
    .expect("the bundled grammars match the tree-sitter version they are built for");
  parser.parse(source, None)
}

fn collect(node: Node<'_>, source: &[u8], classify: Classify, scope: Scope, out: &mut Vec<Item>) {
  let mut cursor = node.walk();
  for child in node.named_children(&mut cursor) {
    match classify(child, source, scope) {
      Visit::Define {
        kind,
        name,
        signature,
        body,
      } => {
        let mut children = Vec::new();
        if let Some(body) = body {
          collect(body, source, classify, Scope::In(kind), &mut children);
        }
        out.push(Item {
          kind,
          name,
          signature,
          start: child.start_position().row + 1,
          end: child.end_position().row + 1,
          children,
        });
      }
      Visit::Descend => collect(child, source, classify, scope, out),
      Visit::Skip => {}
    }
  }
}

/// The text of `node`, on one line.
fn text(node: Node<'_>, source: &[u8]) -> String {
  one_line(&source[node.byte_range()])
}

/// The text of `node` up to byte `end` (from the start of the source), on one line, without the
/// `{`, `=`, `:` or `;` that opens its body or ends it, and at most [`SIGNATURE_LIMIT`] long.
fn signature(node: Node<'_>, source: &[u8], end: usize) -> String {
  let end = end.clamp(node.start_byte(), node.end_byte());
  let line = one_line(&source[node.start_byte()..end]);
  let trimmed = line.trim_end_matches(['{', '=', ':', ';', ' ']);
  match trimmed.char_indices().nth(SIGNATURE_LIMIT) {
    Some((cut, _)) => format!("{}…", &trimmed[..cut]),
    None => trimmed.to_owned(),
  }
}

/// `node`'s signature up to the start of its field `field`, or all of it without that field.
fn signature_before(node: Node<'_>, source: &[u8], field: &str) -> String {
  let end = node
    .child_by_field_name(field)
    .map_or(node.end_byte(), |part| part.start_byte());
  signature(node, source, end)
}

/// `bytes` as text on one line: every run of whitespace, line breaks included, as one space, and
/// none inside the brackets of a list that was written one item per line.
fn one_line(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes)
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .replace("( ", "(")
    .replace(", )", ")")
    .replace(" )", ")")
}

/// A definition of `node` named by its field `name`, with signature `signature` and nested
/// definitions in `body`; skipped when it has no name.
fn define<'t>(node: Node<'t>, source: &[u8], kind: Kind, signature: String, body: Option<Node<'t>>) -> Visit<'t> {
  match node.child_by_field_name("name") {
    Some(name) => Visit::Define {
      kind,
      name: text(name, source),
      signature,
      body,
    },
    None => Visit::Skip,
  }
}
