//! `browser_read_page` and `browser_page_text`: a page as its accessibility tree, each element the agent can act on
//! named with a ref, or as its main text. Both are cut to the characters the call allows, and say so.

use std::collections::HashMap;

use orchester_core::orchester_tools::browser::PAGE_DATA_NOTICE;
use serde_json::{Value, json};

use super::ToolError;
use crate::browser::Tab;

/// The roles of the elements an agent can act on, which carry a ref.
const INTERACTIVE: &[&str] = &[
  "button",
  "checkbox",
  "combobox",
  "link",
  "listbox",
  "menuitem",
  "menuitemcheckbox",
  "menuitemradio",
  "option",
  "radio",
  "searchbox",
  "slider",
  "spinbutton",
  "switch",
  "tab",
  "textbox",
  "treeitem",
];
/// Roles that only group others: shown through their children, unless they have a name.
const GROUPING: &[&str] = &["generic", "none", "presentation", "InlineTextBox", "LineBreak"];
/// The states an element's line shows when they are set.
const STATES: &[&str] = &["checked", "disabled", "expanded", "selected", "required", "level"];
/// The longest name or value a line shows.
const MAX_NAME_CHARS: usize = 200;

/// The script that reads a page's main text: its article, else its main element, else its body.
const MAIN_TEXT: &str = "(() => { const root = document.querySelector('article') || document.querySelector('main') \
                         || document.body; return root ? root.innerText : ''; })()";

/// One line of the tree, before refs are given.
struct Line {
  depth: usize,
  text: String,
  /// The element's `backendNodeId`, when the agent can act on it.
  node: Option<i64>,
}

/// `tab`'s accessibility tree as indented lines; only the elements whose name holds `query`, and only those the agent
/// can act on when `interactive_only`. This read's refs replace the tab's earlier ones.
pub(super) async fn read(
  tab: &Tab,
  query: Option<&str>,
  interactive_only: bool,
  max_chars: u32,
) -> Result<String, ToolError> {
  let tree = tab.call("Accessibility.getFullAXTree", json!({})).await?;
  let nodes = tree["nodes"].as_array().map_or(&[][..], Vec::as_slice);
  let query = query.map(str::to_lowercase);
  let lines: Vec<Line> = walk(nodes)
    .into_iter()
    .filter(|line| !interactive_only || line.node.is_some())
    .filter(|line| {
      query
        .as_ref()
        .is_none_or(|query| line.text.to_lowercase().contains(query))
    })
    .collect();
  let refs = tab.name_refs(&lines.iter().filter_map(|line| line.node).collect::<Vec<_>>());
  let mut refs = refs.into_iter();
  let mut shown = String::new();
  for line in &lines {
    let indent = if query.is_some() || interactive_only {
      0
    } else {
      line.depth
    };
    shown.push_str(&"  ".repeat(indent));
    shown.push_str(&line.text);
    if line.node.is_some()
      && let Some(name) = refs.next()
    {
      shown.push_str(&format!(" [{name}]"));
    }
    shown.push('\n');
  }
  if lines.is_empty() {
    shown.push_str("(no element matches)\n");
  }
  let info = tab.info().await?;
  Ok(format!(
    "{PAGE_DATA_NOTICE}\nTab {}: \"{}\" {}\n{}",
    info.id,
    info.title,
    info.url,
    within(&shown, max_chars, "with query or interactive_only")
  ))
}

/// The main text of `tab`'s page.
pub(super) async fn text(tab: &Tab, max_chars: u32) -> Result<String, ToolError> {
  let answer = tab
    .call(
      "Runtime.evaluate",
      json!({ "expression": MAIN_TEXT, "returnByValue": true }),
    )
    .await?;
  let text = answer["result"]["value"].as_str().unwrap_or_default();
  let info = tab.info().await?;
  Ok(format!(
    "{PAGE_DATA_NOTICE}\nTab {}: \"{}\" {}\n{}",
    info.id,
    info.title,
    info.url,
    within(text, max_chars, "with a larger max_chars")
  ))
}

/// The tree of `nodes` (`Accessibility.getFullAXTree`) in document order, from its root, without ignored nodes and
/// unnamed groups, whose children take their place.
fn walk(nodes: &[Value]) -> Vec<Line> {
  let by_id: HashMap<&str, &Value> = nodes
    .iter()
    .filter_map(|node| Some((node["nodeId"].as_str()?, node)))
    .collect();
  let Some(root) = nodes.first() else {
    return Vec::new();
  };
  let mut lines = Vec::new();
  // Each entry: the node, its depth, and the name of the line above it, which a text repeating it leaves out.
  let mut stack: Vec<(&Value, usize, &str)> = vec![(root, 0, "")];
  while let Some((node, depth, above)) = stack.pop() {
    let (depth_below, name_below) = match line_of(node, above) {
      Some(text) => {
        let role = node["role"]["value"].as_str().unwrap_or_default();
        lines.push(Line {
          depth,
          text,
          node: node["backendDOMNodeId"]
            .as_i64()
            .filter(|_| INTERACTIVE.contains(&role)),
        });
        (depth + 1, node["name"]["value"].as_str().unwrap_or_default())
      }
      None => (depth, above),
    };
    let children = node["childIds"].as_array().map_or(&[][..], Vec::as_slice);
    for child in children.iter().rev() {
      if let Some(child) = child.as_str().and_then(|id| by_id.get(id)) {
        stack.push((child, depth_below, name_below));
      }
    }
  }
  lines
}

/// How `node` reads as a line: `- role "name"` with its value and states; `None` for a node shown only through its
/// children, and for a text that repeats the line above it.
fn line_of(node: &Value, above: &str) -> Option<String> {
  let role = node["role"]["value"].as_str().unwrap_or_default();
  let name = node["name"]["value"].as_str().unwrap_or_default().trim();
  if node["ignored"].as_bool() == Some(true) || (GROUPING.contains(&role) && name.is_empty()) {
    return None;
  }
  if role == "StaticText" {
    return (!name.is_empty() && name != above.trim()).then(|| format!("- text \"{}\"", cut(name)));
  }
  let mut line = format!("- {role}");
  if !name.is_empty() {
    line.push_str(&format!(" \"{}\"", cut(name)));
  }
  if let Some(value) = node["value"]["value"].as_str().filter(|value| !value.is_empty()) {
    line.push_str(&format!(" value=\"{}\"", cut(value)));
  }
  for property in node["properties"].as_array().into_iter().flatten() {
    let state = property["name"].as_str().unwrap_or_default();
    if STATES.contains(&state) {
      line.push_str(&state_of(state, &property["value"]["value"]));
    }
  }
  Some(line)
}

/// How a line shows `state` with `value`: ` [checked]`, ` [level=2]`; nothing while it is not set.
fn state_of(state: &str, value: &Value) -> String {
  match value {
    Value::Bool(true) => format!(" [{state}]"),
    Value::Bool(false) | Value::Null => String::new(),
    Value::String(value) if value == "false" => String::new(),
    Value::String(value) => format!(" [{state}={value}]"),
    value => format!(" [{state}={value}]"),
  }
}

/// `name` within [`MAX_NAME_CHARS`], on one line.
fn cut(name: &str) -> String {
  let flat = name.replace(['\n', '\r'], " ");
  match flat.char_indices().nth(MAX_NAME_CHARS) {
    Some((end, _)) => format!("{}…", &flat[..end]),
    None => flat,
  }
}

/// `text` within `max_chars` characters, saying how to see more when it was cut (`narrow`).
fn within(text: &str, max_chars: u32, narrow: &str) -> String {
  let limit = usize::try_from(max_chars).unwrap_or(usize::MAX);
  match text.char_indices().nth(limit) {
    Some((end, _)) => {
      let left_out = text[end..].chars().count();
      format!(
        "{}\n[cut: {left_out} more characters were left out; read more {narrow}]",
        &text[..end]
      )
    }
    None => text.to_owned(),
  }
}
