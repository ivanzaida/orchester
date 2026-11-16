//! A press only a person makes, sent by an MCP client through the app's own server and input gate: the client's tool
//! call answers with an error that names the action, and the window stays as it was; other presses go through as
//! before, and a person's press still works. `cargo test -p orchester-app --test files person_only`.
use orchester_workspace::files::Version;
use serde_json::{Value, json};

use super::{harness::Files, mcp_client::Client};

/// The ref `lurq_find_by_id` hands out for the element `id`.
fn reference(client: &Client, files: &mut Files, id: &str) -> String {
  let found = client.call(&mut files.window, "lurq_find_by_id", &json!({ "id": id }));
  let found: Value = serde_json::from_str(&found).expect("a tool result");
  found["content"][0]["text"]
    .as_str()
    .and_then(|text| text.split_whitespace().next())
    .unwrap_or_else(|| panic!("a ref for {id}: {found}"))
    .to_owned()
}

/// Whether a tool's answer is an error, and its words.
fn answer(said: &str) -> (bool, String) {
  let said: Value = serde_json::from_str(said).expect("a tool result");
  let error = said["isError"].as_bool().unwrap_or(false);
  let words = said["content"][0]["text"].as_str().unwrap_or_default().to_owned();
  (error, words)
}

#[test]
fn files_person_only_a_clients_press_on_copy_is_its_tool_calls_error_and_changes_nothing() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  let client = Client::new(&mut files.window);
  let copy = reference(&client, &mut files, "files-copy");

  let said = client.call(
    &mut files.window,
    "lurq_act",
    &json!({ "ref": copy, "action": "invoke" }),
  );
  let (error, words) = answer(&said);
  assert!(error, "the call answers with an error: {said}");
  assert!(words.contains("person_only"), "{words}");
  assert!(
    words.contains("copy a file's text from the code viewer"),
    "it names the action: {words}"
  );
  assert!(
    words.contains("Only a person can"),
    "and that a person does it: {words}"
  );
  assert!(words.contains("No setting lets automation do it"), "{words}");
  files.window.pass();
  assert!(files.stand_ins.copied.lock().is_empty(), "nothing was copied");
  assert!(
    files.window.texts("files-refusal").is_none(),
    "the page says nothing of it"
  );
  assert_eq!(files.window.refused_press(), None, "the call answered for its refusal");

  // A press that is not a person's alone goes through the same server as before.
  let find = reference(&client, &mut files, "files-find-open");
  let said = client.call(
    &mut files.window,
    "lurq_act",
    &json!({ "ref": find, "action": "invoke" }),
  );
  assert!(!answer(&said).0, "an ordinary press is no error: {said}");
  files.window.until("the find field", |w| {
    w.tree.get_element_by_id("files-find-field").is_some()
  });

  // The person's own press copies.
  files.window.press("files-copy");
  files.window.pass();
  assert!(!files.stand_ins.copied.lock().is_empty(), "a person's press copies");
}

#[test]
fn files_person_only_text_a_client_types_into_find_is_refused_to_it() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  files.window.press("files-find-open");
  let client = Client::new(&mut files.window);
  let field = reference(&client, &mut files, "files-find-field");
  let clicked = client.call(
    &mut files.window,
    "lurq_interact",
    &json!({ "action": "click", "ref": field }),
  );
  assert!(!answer(&clicked).0, "focusing the field is no error: {clicked}");
  let said = client.call(
    &mut files.window,
    "lurq_interact",
    &json!({ "action": "type", "text": "limit" }),
  );
  let (error, words) = answer(&said);
  assert!(error, "{said}");
  assert!(words.contains("search within a file"), "{words}");
  files.window.pass();
  assert_eq!(find_text(&mut files), "", "typed text never lands in the field");

  // A value set straight into the field is refused to its call too, and the field takes back what it held.
  let said = client.call(
    &mut files.window,
    "lurq_set_value",
    &json!({ "ref": field, "value": "limit" }),
  );
  let (error, words) = answer(&said);
  assert!(error, "{said}");
  assert!(words.contains("search within a file"), "{words}");
  files.window.pass();
  assert_eq!(find_text(&mut files), "", "the field keeps what it held");

  // The person's own typing is searched.
  files.type_keys("x");
  assert_eq!(find_text(&mut files), "x");
}

/// What the find field holds.
fn find_text(files: &mut Files) -> String {
  files
    .window
    .tree
    .get_element_by_id_mut("files-find-field")
    .and_then(|element| element.as_text_input())
    .map(|input| input.value())
    .expect("the find field is a text input")
}
