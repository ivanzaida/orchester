//! A file's text never reaches the MCP tree: automation driving the window reads file names and the tree, never
//! what a file holds. The MCP client is in process, over the production tools and input gate.
use orchester_app::routes::FilesView;
use orchester_workspace::files::{LineRange, Version};
use serde_json::{Value, json};

use super::{
  harness::{Files, MARKER},
  mcp_client::Client,
};

#[test]
fn files_mcp_tree_shows_names_and_never_a_files_text() {
  let mut files = Files::new();
  files.go(FilesView::new(Some("src/main.rs"), Version::Checkout).with_line(LineRange::new(1, 3)));
  files
    .window
    .until("the first line", |w| w.tree.get_element_by_id("files-line-1").is_some());
  let client = Client::new(&mut files.window);
  let tree = client.call(&mut files.window, "lurq_read_tree", &json!({}));
  assert!(tree.contains("main.rs"), "the file's name is visible");
  assert!(tree.contains("files-tree:src/ui"), "the tree is visible");
  assert!(tree.contains("files-line-1"), "the lines are in the tree");
  assert!(tree.contains("sensitive"), "the code is marked sensitive");
  for text in [MARKER, "let value_", "fn main"] {
    assert!(!tree.contains(text), "the file's text {text:?} reached the MCP tree");
  }
  let found = client.call(&mut files.window, "lurq_find", &json!({"query": MARKER}));
  assert!(
    !found.contains("files-line"),
    "a search of the tree finds no line of the file: {found}"
  );
  let inspected = client.call(&mut files.window, "lurq_inspect", &json!({"query": "files-line-2"}));
  for text in [MARKER, "let value_", "fn main"] {
    assert!(
      !inspected.contains(text),
      "the file's text {text:?} reached an inspection"
    );
  }
}

#[test]
fn files_find_counts_nothing_that_automation_types() {
  let mut files = Files::new();
  files.go(FilesView::new(Some("src/main.rs"), Version::Checkout));
  files.window.press("files-find-open");
  let client = Client::new(&mut files.window);
  let field = client.call(&mut files.window, "lurq_find_by_id", &json!({"id": "files-find-field"}));
  let field: Value = serde_json::from_str(&field).expect("a tool result");
  let reference = field["content"][0]["text"]
    .as_str()
    .and_then(|text| text.split_whitespace().next())
    .expect("a ref")
    .to_owned();
  client.call(
    &mut files.window,
    "lurq_set_value",
    &json!({"ref": reference, "value": "limit"}),
  );
  files.window.pass();
  let shown = files.window.text("files-find-field-box");
  assert!(
    !shown.contains(" of ") && !shown.contains("No results"),
    "automation's text is not counted: {shown}"
  );
  files.type_keys("x");
  files.window.until("a person's typing counted", |w| {
    w.text("files-find-field-box").contains("No results")
  });
}
