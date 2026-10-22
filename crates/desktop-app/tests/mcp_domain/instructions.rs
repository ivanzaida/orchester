//! Instructions through the domain tools: saved and published by the editor's own checked writes, so a
//! rule is refused exactly as Publish refuses it — a variable without a value at one of the rule's events
//! — whatever the project's tasks and runs are. Publishing reaches agents' prompts, so automation publishes only
//! under *Let automation change agent permissions* (ORC-30); a draft it saved is marked as automation's.

use orchester_app::routes::{InstructionsTab, InstructionsView, ProjectPage, Route};
use orchester_core::resource::{Instruction, PromptText};
use orchester_workspace::instructions::{data::ProjectInstructions, editor};
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

/// The instruction `name` as list_instructions lists it.
fn listed(served: &mut Served, name: &str) -> Value {
  let listed = served.ok("list_instructions", &json!({ "project": "Kontur" }));
  listed["instructions"]
    .as_array()
    .and_then(|rules| rules.iter().find(|rule| rule["name"] == name))
    .cloned()
    .expect("listed")
}

#[test]
fn domain_publishing_an_instruction_needs_a_person_or_their_switch() {
  let (mut served, _) = Served::seeded("empty");
  let saved = served.ok("save_instruction_draft", &rule("Steer", "Push to main.", "start"));
  assert_eq!(saved["instruction"]["draft_by_automation"], true, "{saved}");
  let refused = served.refused("publish_instruction", &publish("Steer"));
  assert_eq!(refused["code"], "person_only", "{refused}");
  let reasons = refused["reasons"].to_string();
  assert!(
    reasons.contains("changes what agents are told") && reasons.contains("prompts of every agent"),
    "{refused}"
  );
  let steer = listed(&mut served, "Steer");
  assert_eq!(steer["state"], "draft", "nothing was published: {steer}");
  assert_eq!(steer["published_version"], Value::Null, "{steer}");

  served.let_automation_widen(true);
  let published = served.ok("publish_instruction", &publish("Steer"));
  assert_eq!(published["instruction"]["published_version"], 1, "{published}");
  assert_eq!(published["instruction"]["draft_by_automation"], false, "{published}");
}

#[test]
fn domain_a_draft_a_person_saves_after_automation_is_theirs() {
  let (mut served, seeded) = Served::seeded("empty");
  served.ok("save_instruction_draft", &rule("Shared", "First words.", "start"));
  let data = ProjectInstructions::load_in(&seeded.repo).expect("read");
  let entry = data
    .entries
    .iter()
    .find(|entry| &*entry.name == "Shared")
    .expect("saved")
    .clone();
  assert!(data.by_automation(&entry), "automation wrote it");
  let body = Instruction::new(
    PromptText::new("A person's words.").expect("text"),
    entry.body.targets().clone(),
    entry.body.events().clone(),
    entry.body.scope().clone(),
  )
  .expect("a body");
  editor::save_in(&seeded.repo, &entry, body).expect("a person saves it");
  assert_eq!(listed(&mut served, "Shared")["draft_by_automation"], false);
}

#[test]
fn domain_window_input_publishes_an_instruction_only_under_the_persons_switch() {
  let (mut s, project, repo) = super::connections_window::controlled();
  let saved = s.ok("save_instruction_draft", &rule("Steer", "Push to main.", "start"));
  let id = saved["instruction"]["id"]
    .as_str()
    .expect("an id")
    .parse()
    .expect("a resource id");
  s.until("navigation attached", |s| s.link.navigation().is_ok());
  s.link.navigation().expect("navigation").go(Route::Project(
    project,
    ProjectPage::Instructions(InstructionsView::rule(InstructionsTab::Project, Some(id))),
  ));
  s.until("the editor", |s| s.shows("instruction-publish"));
  let published = || {
    ProjectInstructions::load_in(&repo)
      .expect("read")
      .entries
      .iter()
      .any(|entry| &*entry.name == "Steer" && entry.state.published().is_some())
  };
  s.click_by_client("instruction-publish")
    .expect("the click is delivered");
  for _ in 0..10 {
    s.pass();
  }
  assert!(
    !published(),
    "an MCP client's press publishes nothing without the person's switch"
  );
  s.click_by_person("instruction-publish");
  s.until("the person's publication", |_| published());
}

#[test]
fn domain_window_input_saves_a_draft_marked_as_automations() {
  let (mut s, project, repo) = super::connections_window::controlled();
  s.ok("save_instruction_draft", &rule("Typed", "First words.", "start"));
  let entry = |repo: &std::path::Path| {
    let data = ProjectInstructions::load_in(repo).expect("read");
    let entry = data
      .entries
      .iter()
      .find(|entry| &*entry.name == "Typed")
      .expect("saved")
      .clone();
    let automated = data.by_automation(&entry);
    (entry, automated)
  };
  // A person's save makes the draft theirs, so what the window saves next decides the mark.
  let (saved, _) = entry(&repo);
  let body = Instruction::new(
    PromptText::new("A person's words.").expect("text"),
    saved.body.targets().clone(),
    saved.body.events().clone(),
    saved.body.scope().clone(),
  )
  .expect("a body");
  editor::save_in(&repo, &saved, body).expect("a person saves it");
  assert!(!entry(&repo).1, "the person's draft");
  s.until("navigation attached", |s| s.link.navigation().is_ok());
  s.link.navigation().expect("navigation").go(Route::Project(
    project,
    ProjectPage::Instructions(InstructionsView::rule(InstructionsTab::Project, Some(saved.id))),
  ));
  s.until("the editor", |s| s.shows("instruction-prompt"));
  s.set_by_client("instruction-prompt", "A client's words.");
  s.until("the client's draft saved", |_| {
    entry(&repo).0.body.text().as_str() == "A client's words."
  });
  assert!(
    entry(&repo).1,
    "a draft an MCP client typed in the window is marked as automation's"
  );
}

#[test]
fn domain_instructions_publish_checks_values_at_the_rules_events() {
  let (mut served, _) = Served::seeded("empty");
  served.let_automation_widen(true);
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
  let checkpoint = listed(&mut served, "Checkpoint");
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
  served.let_automation_widen(true);
  served.ok("save_instruction_draft", &rule("Blank", "", "start"));
  let error = served.refused("publish_instruction", &publish("Blank"));
  assert_eq!(error["code"], "blank_text", "{error}");
  let mut renamed = rule("Other", "Text", "start");
  renamed["instruction"] = json!("Blank");
  let error = served.refused("save_instruction_draft", &renamed);
  assert_eq!(error["code"], "name_not_editable", "{error}");
}
