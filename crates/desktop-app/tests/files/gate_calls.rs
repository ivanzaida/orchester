//! How the input gate ties a person-only press's refusal to the MCP call that made it: window-input calls take turns,
//! two clients each get their own refusal, a refusal made while no call waits leaks into no later call, and a call
//! whose input runs no handler answers as before. `cargo test -p orchester-app --test files gate_calls`.
use lurq::app::events::with_synthetic_input;
use orchester_workspace::files::Version;
use serde_json::{Value, json};

use super::{harness::Files, mcp_client::Client};

const COPY: &str = "copy a file's text from the code viewer";
const OPEN: &str = "open a file in its default program";

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

/// The action a tool's answer refused, or `None` when it is no refusal.
fn refused(said: &str) -> Option<String> {
  let said: Value = serde_json::from_str(said).expect("a tool result");
  if !said["isError"].as_bool().unwrap_or(false) {
    return None;
  }
  let words = said["content"][0]["text"].as_str().expect("words");
  let refusal: Value = serde_json::from_str(words).expect("a refusal");
  assert_eq!(refusal["code"], "person_only", "{words}");
  Some(refusal["action"].as_str().expect("the action").to_owned())
}

fn invoke(reference: &str) -> Value {
  json!({ "ref": reference, "action": "invoke" })
}

#[test]
fn gate_calls_two_clients_pressing_at_once_each_get_their_own_refusal_in_turn() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  let first = Client::new(&mut files.window);
  let second = first.session(&mut files.window);
  let (copy, open) = (
    reference(&first, &mut files, "files-copy"),
    reference(&second, &mut files, "files-open"),
  );
  let calls = vec![
    first.start(&files.window, "lurq_act", &invoke(&copy)),
    second.start(&files.window, "lurq_act", &invoke(&open)),
    first.start(&files.window, "lurq_act", &invoke(&copy)),
  ];
  let said = Client::answers(&mut files.window, calls);
  assert_eq!(refused(&said[0]).as_deref(), Some(COPY), "{}", said[0]);
  assert_eq!(refused(&said[1]).as_deref(), Some(OPEN), "{}", said[1]);
  assert_eq!(refused(&said[2]).as_deref(), Some(COPY), "{}", said[2]);
  assert!(files.stand_ins.copied.lock().is_empty(), "nothing was copied");
}

#[test]
fn gate_calls_a_refusal_made_while_no_call_waits_is_answered_to_no_later_call() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  let client = Client::new(&mut files.window);
  let find = reference(&client, &mut files, "files-find-open");
  // Synthetic input no MCP call delivered: its refusal has no call to answer.
  with_synthetic_input(|| files.window.press("files-copy"));
  let said = client.call(&mut files.window, "lurq_act", &invoke(&find));
  assert_eq!(refused(&said), None, "the next call is not refused for it: {said}");
  assert_eq!(files.window.refused_press(), None, "and the refusal was let go");
  files.window.until("the find field", |w| {
    w.tree.get_element_by_id("files-find-field").is_some()
  });
}

#[test]
fn gate_calls_a_call_whose_input_runs_no_handler_answers_as_before() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  let client = Client::new(&mut files.window);
  for arguments in [
    json!({ "action": "move", "x": 4, "y": 4 }),
    json!({ "action": "click", "x": 4, "y": 4 }),
    json!({ "action": "key", "key": "Shift" }),
  ] {
    let said = client.call(&mut files.window, "lurq_interact", &arguments);
    assert_eq!(refused(&said), None, "{arguments}: {said}");
  }
  let copy = reference(&client, &mut files, "files-copy");
  let said = client.call(&mut files.window, "lurq_act", &invoke(&copy));
  assert_eq!(
    refused(&said).as_deref(),
    Some(COPY),
    "a person-only press after them is still refused: {said}"
  );
}
