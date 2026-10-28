//! Native agent connections through the domain tools hold the window's rules (`docs/architecture/native-agent.md`
//! §7): their models are the ones they store, with each model's efforts; a profile on one takes only an effort its
//! model lists; and `check_connection` tests one through the project's runner, as *Test connection* does, instead of
//! saying native agents are not available.

use serde_json::{Value, json};

use super::Served;

const NATIVE: &str = "Anthropic API · Kontur";

fn on_native() -> Value {
  json!({ "project": "Kontur", "connection": NATIVE })
}

#[test]
fn domain_native_models_are_the_stored_ones() {
  let (mut served, _seeded) = Served::seeded("native");
  let listed = served.ok("list_models", &on_native());
  assert_eq!(listed["source"], "native", "{listed}");
  assert_eq!(
    listed["models"],
    json!([
      { "id": "claude-opus-5-5", "efforts": ["low", "medium", "high", "max"] },
      { "id": "claude-sonnet-5-5", "efforts": ["low", "medium", "high"] },
      { "id": "claude-review-preview", "efforts": [] },
    ]),
    "{listed}"
  );
  let connections = served.ok("list_connections", &json!({ "project": "Kontur" }));
  let native = connections["agents"]
    .as_array()
    .expect("agent connections")
    .iter()
    .find(|connection| connection["name"] == NATIVE)
    .unwrap_or_else(|| panic!("{NATIVE} is listed: {connections}"));
  assert_eq!(native["models"], 3, "{native}");
}

#[test]
fn domain_native_profile_takes_only_an_effort_its_model_lists() {
  let (mut served, _seeded) = Served::seeded("native");
  let builder = |effort: &str| {
    json!({
      "project": "Kontur",
      "profile": "Builder",
      "role": "Builder",
      "connection": NATIVE,
      "model": "claude-opus-5-5",
      "effort": effort,
      "permissions": { "repository": "write", "commands": "ask_before", "network": "off" },
    })
  };
  let refused = served.refused("save_profile_draft", &builder("bogus"));
  assert_eq!(refused["code"], "effort_not_offered", "{refused}");
  let saved = served.ok("save_profile_draft", &builder("max"));
  assert_eq!(saved["profile"]["role"], "Builder", "{saved}");
}

#[test]
fn domain_native_check_goes_to_the_runner() {
  let (mut served, _seeded) = Served::seeded("native");
  // The seeded project's runner program does not exist: the test is asked of a runner that is not there.
  let refused = served.refused("check_connection", &on_native());
  assert_eq!(refused["code"], "not_checked", "{refused}");
  let reasons = refused["reasons"].to_string();
  assert!(reasons.contains("runner"), "{refused}");
  assert!(!reasons.contains("not available in this version"), "{refused}");
}
