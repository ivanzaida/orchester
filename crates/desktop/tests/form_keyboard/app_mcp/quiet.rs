//! Tools change data in the background without disturbing the person in the window: the task page
//! shows a changed task in place; an Edit task form keeps its edits and says the task changed, or
//! takes the new values while it has none, and a save keeps the tool's change; a task a tool queued
//! but could not start stays on its page with Retry run; and only the changed project's data counts
//! as changed. Run with `cargo test -p orchester-desktop --test form_keyboard app_mcp_quiet`.

use orchester_core::{id::ProjectId, store::TaskStore, task::Task};
use orchester_desktop::tasks::outside::{Outside, OutsideChanges};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{agent, ok, refused};
use crate::{
  github::shows,
  harness::Desktop,
  mcp_agent::McpAgent,
  task_run::{has, seed, set_text},
};

const ROW: &str = "task-row-KON-1";

/// Kontur with the published pipeline Delivery and the draft KON-1 a tool created, its page open.
fn task_page() -> (Desktop, McpAgent) {
  let mut desktop =
    Desktop::with_seeded_project_and_app_mcp(|repo| seed(repo, &repo.join("orchester-fake-codex"), false));
  let mut client = agent(&desktop);
  ok(
    &mut client,
    "create_task",
    &json!({"project": "Kontur", "title": "Add GitHub authentication",
      "brief": "Let users sign in with GitHub.", "criteria": ["A new user can sign in."], "pipeline": "Delivery"}),
  );
  open_task(&mut desktop);
  (desktop, client)
}

fn open_task(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for(ROW);
  desktop.press(ROW);
  desktop.wait_for("task-this-run");
}

fn edit(desktop: &mut Desktop) {
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_for("task-title");
}

fn stored(desktop: &Desktop) -> Task {
  ProjectDb::open(desktop.repo()).expect("open").tasks().expect("tasks")[0].clone()
}

/// `update_task` of KON-1 with the fields of `change`.
fn change(client: &mut McpAgent, change: serde_json::Value) {
  let serde_json::Value::Object(fields) = change else {
    panic!("the fields to change are an object");
  };
  let mut arguments = json!({"project": "Kontur", "key": "KON-1"});
  arguments.as_object_mut().expect("an object").extend(fields);
  ok(client, "update_task", &arguments);
}

#[test]
fn app_mcp_quiet_task_page_shows_a_changed_task_in_place() {
  let (mut desktop, mut client) = task_page();
  change(&mut client, json!({"title": "Sign in with GitHub"}));
  desktop.wait_until("the page reads the task again", |desktop| {
    shows(desktop, "task-heading", "Sign in with GitHub")
  });
  assert!(has(&mut desktop, "task-page"), "the same page");
  assert!(!has(&mut desktop, "task-form"));
}

#[test]
fn app_mcp_quiet_edit_form_keeps_its_edits_and_a_save_keeps_the_tools_change() {
  let (mut desktop, mut client) = task_page();
  edit(&mut desktop);
  set_text(&mut desktop, "task-title", "Sign in with GitHub or GitLab");
  change(&mut client, json!({"brief": "Changed by automation."}));
  desktop.wait_until("the notice", |desktop| has(desktop, "edit-task-changed-outside"));
  assert!(shows(
    &mut desktop,
    "edit-task-changed-outside",
    "Changed outside this form"
  ));
  assert_eq!(
    desktop.value("task-title"),
    "Sign in with GitHub or GitLab",
    "the edit is kept"
  );
  assert!(has(&mut desktop, "task-form"), "the form stays");

  desktop.press("save-draft");
  desktop.wait_until("the save", |desktop| {
    stored(desktop).details().title.as_str() == "Sign in with GitHub or GitLab"
  });
  assert_eq!(
    stored(&desktop).details().brief,
    "Changed by automation.",
    "the save keeps what the tool changed"
  );
  desktop.wait_until("the notice gives way", |desktop| {
    !has(desktop, "edit-task-changed-outside")
  });
}

#[test]
fn app_mcp_quiet_edit_form_without_edits_takes_the_new_values() {
  let (mut desktop, mut client) = task_page();
  edit(&mut desktop);
  change(&mut client, json!({"title": "Sign in with GitHub"}));
  desktop.wait_until("the form shows the stored title", |desktop| {
    desktop.value("task-title") == "Sign in with GitHub"
  });
  assert!(!has(&mut desktop, "edit-task-changed-outside"));
  assert!(has(&mut desktop, "task-form"));
}

#[test]
fn app_mcp_quiet_edit_form_reload_shows_the_stored_task() {
  let (mut desktop, mut client) = task_page();
  edit(&mut desktop);
  set_text(&mut desktop, "task-title", "Mine");
  change(&mut client, json!({"title": "Theirs"}));
  desktop.wait_until("the notice", |desktop| has(desktop, "edit-task-reload"));
  desktop.click("edit-task-reload");
  desktop.wait_until("the stored title", |desktop| desktop.value("task-title") == "Theirs");
  assert!(!has(&mut desktop, "edit-task-changed-outside"));
}

#[test]
fn app_mcp_quiet_task_a_tool_could_not_start_stays_on_its_page() {
  let (mut desktop, mut client) = task_page();
  // No runner runs here: the task is queued, and its run does not start.
  refused(&mut client, "run_task", &json!({"project": "Kontur", "key": "KON-1"}));
  desktop.wait_until("Retry run on the page", |desktop| has(desktop, "task-retry-first-run"));
  assert!(has(&mut desktop, "task-page"));
  assert!(!has(&mut desktop, "task-form"), "not the New task form");
  assert!(shows(&mut desktop, "task-heading", "Queued"));
  // Opened again from the task list, it is the same page.
  open_task(&mut desktop);
  desktop.wait_for("task-retry-first-run");
  assert!(!has(&mut desktop, "task-form"));
}

#[test]
fn app_mcp_quiet_outside_changes_count_only_for_the_changed_project() {
  let changes = OutsideChanges::default();
  let (changed, other) = (ProjectId::generate(), ProjectId::generate());
  let mut updates = changes.subscribe();
  changes.record(changed, Outside::Tasks);
  changes.record(changed, Outside::AutoMode);
  assert!(updates.has_changed().expect("the sender lives"));
  let counts = updates.borrow_and_update().clone();
  assert_eq!(
    counts
      .get(&changed)
      .map(|counts| (counts.tasks, counts.auto_mode, counts.code_index)),
    Some((1, 1, 0)),
    "each setting counts apart"
  );
  assert_eq!(
    counts.get(&other),
    None,
    "another project's feed has nothing to read again"
  );
}
