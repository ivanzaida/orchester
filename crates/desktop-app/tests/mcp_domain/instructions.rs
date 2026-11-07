//! Instructions through the domain tools: a new one is created as v1 in one call (no draft), an existing one is edited
//! as a draft and published, both by the editor's own checked writes, so a rule is refused exactly as Publish refuses
//! it — blank text, a variable without a value at one of the rule's events. Reaching agents' prompts, creating and
//! publishing are open to automation only under *Let automation change agent permissions* (ORC-30); a draft it saved
//! is marked as automation's.

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

/// A change to the existing instruction `name`: its draft.
fn edit(name: &str, text: &str, events: &str) -> Value {
  let mut edit = rule(name, text, events);
  edit["instruction"] = json!(name);
  edit.as_object_mut().expect("an object").remove("name");
  edit
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

/// How many instructions list_instructions lists.
fn count(served: &mut Served) -> usize {
  let listed = served.ok("list_instructions", &json!({ "project": "Kontur" }));
  listed["instructions"].as_array().expect("instructions").len()
}

#[test]
fn domain_creating_an_instruction_needs_a_person_or_their_switch_and_editing_drafts() {
  let (mut served, _) = Served::seeded("empty");
  let refused = served.refused("save_instruction_draft", &rule("Steer", "Push to main.", "start"));
  assert_eq!(refused["code"], "person_only", "{refused}");
  let reasons = refused["reasons"].to_string();
  assert!(reasons.contains("prompts of every agent"), "{refused}");
  assert_eq!(count(&mut served), 0, "nothing was created");

  served.let_automation_widen(true);
  let created = served.ok("save_instruction_draft", &rule("Steer", "Push to main.", "start"));
  assert_eq!(created["instruction"]["state"], "published", "{created}");
  assert_eq!(created["instruction"]["published_version"], 1, "{created}");
  assert_eq!(created["instruction"]["draft_by_automation"], false, "{created}");

  served.let_automation_widen(false);
  let saved = served.ok("save_instruction_draft", &edit("Steer", "Push to a branch.", "start"));
  assert_eq!(saved["instruction"]["draft_by_automation"], true, "{saved}");
  let refused = served.refused("publish_instruction", &publish("Steer"));
  assert_eq!(refused["code"], "person_only", "{refused}");
  let steer = listed(&mut served, "Steer");
  assert_eq!(steer["state"], "draft", "nothing was published: {steer}");
  assert_eq!(steer["published_version"], 1, "{steer}");

  served.let_automation_widen(true);
  let published = served.ok("publish_instruction", &publish("Steer"));
  assert_eq!(published["instruction"]["published_version"], 2, "{published}");
  assert_eq!(published["instruction"]["draft_by_automation"], false, "{published}");
}

#[test]
fn domain_a_draft_a_person_saves_after_automation_is_theirs() {
  let (mut served, seeded) = Served::seeded("empty");
  served.let_automation_widen(true);
  served.ok("save_instruction_draft", &rule("Shared", "First words.", "start"));
  served.ok("save_instruction_draft", &edit("Shared", "Second words.", "start"));
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
  s.let_automation_widen(true);
  s.ok("save_instruction_draft", &rule("Steer", "Push to main.", "start"));
  let saved = s.ok("save_instruction_draft", &edit("Steer", "Push to a branch.", "start"));
  s.let_automation_widen(false);
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
  let second = || {
    ProjectInstructions::load_in(&repo)
      .expect("read")
      .entries
      .iter()
      .any(|entry| &*entry.name == "Steer" && entry.state.published().is_some_and(|number| number.get() == 2))
  };
  s.click_by_client("instruction-publish")
    .expect("the click is delivered");
  for _ in 0..10 {
    s.pass();
  }
  assert!(
    !second(),
    "an MCP client's press publishes nothing without the person's switch"
  );
  s.click_by_person("instruction-publish");
  s.until("the person's publication", |_| second());
}

#[test]
fn domain_window_input_saves_a_draft_marked_as_automations() {
  let (mut s, project, repo) = super::connections_window::controlled();
  s.let_automation_widen(true);
  s.ok("save_instruction_draft", &rule("Typed", "First words.", "start"));
  s.let_automation_widen(false);
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
fn domain_instructions_check_values_at_the_rules_events_when_created_and_published() {
  let (mut served, _) = Served::seeded("empty");
  served.let_automation_widen(true);
  let created = served.ok(
    "save_instruction_draft",
    &rule("Task", "Working on {{task.title}}", "start"),
  );
  assert_eq!(created["instruction"]["published_version"], 1, "{created}");

  let error = served.refused(
    "save_instruction_draft",
    &rule("Checkpoint", "Compare {{run.revision}}", "start"),
  );
  assert_eq!(error["code"], "variable_unavailable", "{error}");
  assert_eq!(count(&mut served), 1, "the refused rule is not created");

  let created = served.ok(
    "save_instruction_draft",
    &rule("Checkpoint", "Compare {{run.revision}}", "resume"),
  );
  assert_eq!(created["instruction"]["published_version"], 1, "{created}");
  assert_eq!(created["instruction"]["events"], json!(["resume"]), "{created}");

  served.ok(
    "save_instruction_draft",
    &edit("Task", "Compare {{run.revision}}", "start"),
  );
  let error = served.refused("publish_instruction", &publish("Task"));
  assert_eq!(error["code"], "variable_unavailable", "{error}");
  let task = listed(&mut served, "Task");
  assert_eq!(task["state"], "draft", "{task}");

  let saved = served.ok(
    "save_instruction_draft",
    &edit("Task", "Compare {{run.revision}}", "resume"),
  );
  assert_eq!(saved["instruction"]["events"], json!(["resume"]), "{saved}");
  let published = served.ok("publish_instruction", &publish("Task"));
  assert_eq!(published["instruction"]["published_version"], 2, "{published}");
}

#[test]
fn domain_instructions_refuse_a_blank_rule_and_a_name_change() {
  let (mut served, _) = Served::seeded("empty");
  served.let_automation_widen(true);
  let error = served.refused("save_instruction_draft", &rule("Blank", "", "start"));
  assert_eq!(error["code"], "blank_text", "{error}");
  assert_eq!(count(&mut served), 0, "a blank rule is not created");

  served.ok("save_instruction_draft", &rule("Blank", "Text", "start"));
  let mut renamed = rule("Other", "Text", "start");
  renamed["instruction"] = json!("Blank");
  let error = served.refused("save_instruction_draft", &renamed);
  assert_eq!(error["code"], "name_not_editable", "{error}");
}
