//! A page's accessibility tree (`Accessibility.getFullAXTree`) in the shape every driver answers in
//! (`orchester_core::orchester_tools::ui::UiElement`): ignored nodes and unnamed groups are left out and their children
//! take their place, at most [`MAX_ELEMENTS`] elements [`MAX_DEPTH`] deep. An element's ref names its DOM node (`b` and
//! its `backendNodeId`), so it stays the same from one snapshot to the next while the node lives, and an action by a
//! ref whose node is gone says so.

use std::collections::HashMap;

use orchester_core::orchester_tools::ui::UiElement;
use serde_json::Value;

/// The most elements one snapshot answers.
const MAX_ELEMENTS: usize = 300;
/// How deep one snapshot answers.
const MAX_DEPTH: usize = 40;
/// Roles that only group others: shown through their children, unless they have a name.
const GROUPING: &[&str] = &["generic", "none", "presentation", "LineBreak", "InlineTextBox"];
/// The longest name or value an element shows.
const MAX_TEXT_CHARS: usize = 200;
/// What starts a ref: the element's DOM node follows.
const REF_PREFIX: char = 'b';

/// The `backendNodeId` `reference` names; `None` for a ref no snapshot of this driver gave.
pub(super) fn node_of(reference: &str) -> Option<i64> {
  reference.strip_prefix(REF_PREFIX)?.parse().ok()
}

/// The page's tree as elements.
pub(super) struct Snapshot<'a> {
  by_id: HashMap<&'a str, &'a Value>,
  root: Option<&'a Value>,
}

impl<'a> Snapshot<'a> {
  /// The snapshot of `nodes`, the answer of `Accessibility.getFullAXTree`, whose first node is the root.
  pub(super) fn of(nodes: &'a [Value]) -> Self {
    Self {
      by_id: nodes
        .iter()
        .filter_map(|node| Some((node["nodeId"].as_str()?, node)))
        .collect(),
      root: nodes.first(),
    }
  }

  /// The whole tree from its root, and whether it was cut. A root the tree leaves out (ignored, or without a DOM node)
  /// is a `document` holding what it shows, by the root's DOM node when it has one.
  pub(super) fn tree(&self) -> (Option<UiElement>, bool) {
    let mut count = 0;
    let Some(root) = self.root else {
      return (None, false);
    };
    let mut shown = self.elements(root, (0, ""), &mut count);
    let truncated = count >= MAX_ELEMENTS;
    let tree = match shown.len() {
      0 => None,
      1 => shown.pop(),
      _ => Some(UiElement {
        reference: format!("{REF_PREFIX}{}", root["backendDOMNodeId"].as_i64().unwrap_or_default()),
        role: "document".to_owned(),
        name: None,
        states: Vec::new(),
        value: None,
        children: shown,
      }),
    };
    (tree, truncated)
  }

  /// The elements whose name, role or value holds `query` in any case, without their children, and whether more
  /// matched than were answered.
  pub(super) fn matches(&self, query: &str) -> (Vec<UiElement>, bool) {
    let (tree, _) = self.tree();
    let query = query.to_lowercase();
    let mut found = Vec::new();
    let mut stack: Vec<UiElement> = tree.into_iter().collect();
    while let Some(mut element) = stack.pop() {
      stack.extend(std::mem::take(&mut element.children).into_iter().rev());
      let holds = |text: Option<&str>| text.is_some_and(|text| text.to_lowercase().contains(&query));
      if holds(element.name.as_deref()) || holds(Some(&element.role)) || holds(element.value.as_deref()) {
        found.push(element);
      }
    }
    let truncated = found.len() > MAX_ELEMENTS;
    found.truncate(MAX_ELEMENTS);
    (found, truncated)
  }

  /// `node` as the elements it shows: itself with its children, or its children in its place; `above` is its depth
  /// and the name of the element above it, which a text repeating it leaves out.
  fn elements(&self, node: &Value, (depth, above): (usize, &str), count: &mut usize) -> Vec<UiElement> {
    if *count >= MAX_ELEMENTS {
      return Vec::new();
    }
    let element = (depth < MAX_DEPTH).then(|| element_of(node, above)).flatten();
    let below = match &element {
      Some(_) => (depth + 1, node["name"]["value"].as_str().unwrap_or_default()),
      None => (depth, above),
    };
    if element.is_some() {
      *count += 1;
    }
    let children: Vec<UiElement> = node["childIds"]
      .as_array()
      .into_iter()
      .flatten()
      .filter_map(|child| child.as_str().and_then(|id| self.by_id.get(id)))
      .flat_map(|child| self.elements(child, below, count))
      .collect();
    match element {
      Some(element) => vec![UiElement { children, ..element }],
      None => children,
    }
  }
}

/// `node` as an element without its children; `None` for a node shown only through its children, one without a DOM
/// node to act on, and a text that repeats the element above it.
fn element_of(node: &Value, above: &str) -> Option<UiElement> {
  let role = node["role"]["value"].as_str().unwrap_or_default();
  let name = node["name"]["value"].as_str().unwrap_or_default().trim();
  if node["ignored"].as_bool() == Some(true) || (GROUPING.contains(&role) && name.is_empty()) {
    return None;
  }
  let backend = node["backendDOMNodeId"].as_i64()?;
  let role = match role {
    "StaticText" if name.is_empty() || name == above.trim() => return None,
    "StaticText" => "text",
    "RootWebArea" => "document",
    other => other,
  };
  let value = match &node["value"]["value"] {
    Value::String(text) if !text.is_empty() => Some(cut(text)),
    Value::Number(number) => Some(number.to_string()),
    _ => None,
  };
  Some(UiElement {
    reference: format!("{REF_PREFIX}{backend}"),
    role: role.to_owned(),
    name: (!name.is_empty()).then(|| cut(name)),
    states: states(node),
    value,
    children: Vec::new(),
  })
}

/// The states `node`'s properties set, in the words the native drivers use.
fn states(node: &Value) -> Vec<String> {
  let mut states = Vec::new();
  for property in node["properties"].as_array().into_iter().flatten() {
    let value = &property["value"]["value"];
    let on = value.as_bool() == Some(true) || value.as_str() == Some("true");
    let state = match (property["name"].as_str().unwrap_or_default(), on) {
      ("disabled", true) => "disabled",
      ("focused", true) => "focused",
      ("readonly", true) => "readonly",
      ("required", true) => "required",
      ("selected", true) => "selected",
      ("checked", true) => "checked",
      ("checked", false) if value.as_str() == Some("mixed") => "mixed",
      ("checked", false) => "unchecked",
      ("expanded", true) => "expanded",
      ("expanded", false) => "collapsed",
      _ => continue,
    };
    states.push(state.to_owned());
  }
  states
}

/// `text` within [`MAX_TEXT_CHARS`], on one line.
fn cut(text: &str) -> String {
  let flat = text.replace(['\n', '\r'], " ");
  match flat.char_indices().nth(MAX_TEXT_CHARS) {
    Some((end, _)) => format!("{}…", &flat[..end]),
    None => flat,
  }
}
