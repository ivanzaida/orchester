//! The daily budget's field over the app's own MCP server: a client's `lurq_set_value` that raises or removes the
//! budget is refused to that very call, as its error, and the field keeps what it held; one that lowers it is taken.
//! `cargo test -p orchester-app --test project_settings limits_mcp`.
use serde_json::{Value, json};

use super::{harness::Settings, mcp_client::Client};

const DAILY: &str = "settings-daily-budget";

/// The ref `lurq_find_by_id` hands out for the element `id`.
fn reference(client: &Client, page: &mut Settings, id: &str) -> String {
  let found = client.call(&mut page.window, "lurq_find_by_id", &json!({ "id": id }));
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
fn limits_mcp_a_clients_raise_is_refused_to_its_own_set_value_and_the_field_stays() {
  let mut page = Settings::new(false);
  page.tab("limits", "settings-default-budget");
  let before = page.value(DAILY);
  let client = Client::new(&mut page.window);
  let field = reference(&client, &mut page, DAILY);

  for raised in ["", "40000000"] {
    let said = client.call(
      &mut page.window,
      "lurq_set_value",
      &json!({ "ref": field, "value": raised }),
    );
    let (error, words) = answer(&said);
    assert!(error, "{raised:?}: the set_value call itself is refused: {said}");
    assert!(words.contains("raise or remove a project's daily budget"), "{words}");
    assert!(
      words.contains("Let automation answer approvals"),
      "it names the setting: {words}"
    );
    page.window.pass();
    assert_eq!(page.value(DAILY), before, "{raised:?}: the field keeps what it held");
  }

  // Lowering it is anyone's: the same call is taken, and the next call answers for nothing of the refusals.
  let said = client.call(
    &mut page.window,
    "lurq_set_value",
    &json!({ "ref": field, "value": "10000000" }),
  );
  assert!(!answer(&said).0, "a lower budget is taken: {said}");
  page.window.pass();
  assert_ne!(page.value(DAILY), before, "the field holds the client's lower budget");
}
