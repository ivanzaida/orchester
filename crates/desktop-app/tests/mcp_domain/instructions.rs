//! Instructions through the domain tools: saved and published by the editor's own checked writes, so a
//! rule is refused exactly as Publish refuses it — a variable without a value at one of the rule's events
//! — whatever the project's tasks and runs are.

use serde_json::{Value, json};

use super::Served;

fn rule(name: &str, text: &str, events: &str) -> Value {
  json!({
    "project": "Kontur",
    "name": name,
    "applies_in": "project",
    "agents": "all",
    "events": events,
    "text": text,
  })
}

fn publish(name: &str) -> Value {
  json!({ "project": "Kontur", "instruction": name })
}

#[test]
fn domain_instructions_publish_checks_values_at_the_rules_events() {
  let (mut served, _) = Served::seeded("empty");
  served.ok(
    "save_instruction_draft",
    &rule("Task", "Working on {{task.title}}", "start"),
  );
  let published = served.ok("publish_instruction", &publish("Task"));
  assert_eq!(published["instruction"]["published_version"], 1, "{published}");

  served.ok(
    "save_instruction_draft",
    &rule("Checkpoint", "Compare {{run.revision}}", "start"),
  );
  let error = served.refused("publish_instruction", &publish("Checkpoint"));
  assert_eq!(error["code"], "variable_unavailable", "{error}");
  let listed = served.ok("list_instructions", &json!({ "project": "Kontur" }));
  let checkpoint = listed["instructions"]
    .as_array()
    .and_then(|rules| rules.iter().find(|rule| rule["name"] == "Checkpoint"))
    .expect("listed");
  assert_eq!(checkpoint["state"], "draft", "{checkpoint}");

  let resumed = json!({
    "project": "Kontur",
    "instruction": "Checkpoint",
    "applies_in": "project",
    "agents": "all",
    "events": "resume",
    "text": "Compare {{run.revision}}",
  });
  let saved = served.ok("save_instruction_draft", &resumed);
  assert_eq!(saved["instruction"]["events"], json!(["resume"]), "{saved}");
  let published = served.ok("publish_instruction", &publish("Checkpoint"));
  assert_eq!(published["instruction"]["published_version"], 1, "{published}");
}

#[test]
fn domain_instructions_refuse_a_blank_rule_and_a_name_change() {
  let (mut served, _) = Served::seeded("empty");
  served.ok("save_instruction_draft", &rule("Blank", "", "start"));
  let error = served.refused("publish_instruction", &publish("Blank"));
  assert_eq!(error["code"], "blank_text", "{error}");
  let mut renamed = rule("Other", "Text", "start");
  renamed["instruction"] = json!("Blank");
  let error = served.refused("save_instruction_draft", &renamed);
  assert_eq!(error["code"], "name_not_editable", "{error}");
}
