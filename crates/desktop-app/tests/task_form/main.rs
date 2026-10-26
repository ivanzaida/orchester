//! New task and Edit task (`screens::task_form`) headless at 1440 × 900 on seeded Kontur projects whose
//! runner never starts: the action bar names the one thing the next action needs in the design's words; the
//! limits read back and refuse only what is not a number, with no maximum; Create saves a draft and highlights
//! it in Tasks; Create & run without a runner leaves the task queued with Retry run, which never makes a second
//! task or run; Edit task saves only a change, discards in place, keeps unsaved changes when left, and offers no
//! Create & run for a task that ran; the chips save what the task's runs deliver and its priority (`chips.rs`).
//! Needs `git` (the seed).

mod chips;
mod conflicts;
mod harness;
#[allow(
  dead_code,
  reason = "the seed's scenarios are shared with its own example; these tests use a part"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
mod merging;
mod sources;
#[path = "../support/mod.rs"]
mod support;
mod versions;

use harness::Form;
use lurq::app::events::MouseButton;
use orchester_app::routes::{ProjectPage, Route, TaskList};
use orchester_core::{
  store::{RunStore, TaskStore},
  task::TaskStatus,
};

const REASON_EMPTY: &str = "To create, add a title. To run, also the outcome, one criterion and a pipeline.";
pub const READY: &str = "Ready to run · the run starts as soon as an agent slot is free.";

#[test]
fn form_new_task_says_what_each_action_needs() {
  let mut form = Form::seeded("live");
  form.new_task();
  assert_eq!(form.status(), "Not saved yet");
  for gone in ["task-form-description", "task-form-project", "task-form-context-reason"] {
    assert!(!form.exists(gone), "{gone} left the form");
  }
  assert_eq!(form.reasons(), vec![REASON_EMPTY.to_owned()]);
  assert_eq!(form.text("task-form-pipeline-value"), "Choose");
  assert!(
    !form.exists("task-form-plan"),
    "no plan line before a pipeline is chosen"
  );
  form.set("task-form-title-input", "Add a delivery log");
  form.choose_pipeline("Quick fix v1");
  // Limits are optional: a budget without a time limit, or neither, holds nothing back.
  let without_limits = vec!["To run, describe the outcome and add one criterion.".to_owned()];
  assert_eq!(form.reasons(), without_limits);
  form.limits("5000", "");
  assert_eq!(form.reasons(), without_limits);
  assert_eq!(form.text("task-form-limits-value"), "5k tokens");
  assert!(form.exists("task-form-plan"), "the chosen pipeline's plan line shows");
  assert_eq!(form.text("task-form-pipeline-value"), "Quick fix v1");
}

#[test]
fn form_limits_read_back_and_refuse_only_what_is_not_a_number() {
  let mut form = Form::seeded("live");
  form.new_task();
  assert_eq!(form.text("task-form-limits-value"), "No limit");
  form.open("task-form-limits");
  assert_eq!(
    form.text("task-form-budget-input-read-back"),
    "No limit · the run uses what it needs"
  );
  assert_eq!(
    form.text("task-form-time-limit-input-read-back"),
    "No limit · the run takes the time it needs"
  );
  // A budget set at once reads grouped once the field is left.
  form.set("task-form-budget-input", "1400000");
  focus(&mut form, "task-form-budget-input");
  focus(&mut form, "task-form-time-limit-input");
  assert_eq!(form.value("task-form-budget-input"), "1,400,000");
  form.set("task-form-time-limit-input", "1");
  assert_eq!(
    form.text("task-form-budget-input-read-back"),
    "= 1.4M tokens · input + output"
  );
  assert_eq!(
    form.text("task-form-time-limit-input-read-back"),
    "= 1 minute of active agent time"
  );
  assert_eq!(form.text("task-form-limits-value"), "1.4M · 1 min");
  // No maximum: a budget far above any project's old maximum is a budget.
  form.set("task-form-budget-input", "50,000,000,000");
  assert_eq!(
    form.text("task-form-budget-input-read-back"),
    "= 50B tokens · input + output"
  );
  // More minutes than a time limit can hold is refused in the field, not when the task is saved.
  form.set("task-form-time-limit-input", "307445734561825860");
  assert_eq!(
    form.text("task-form-time-limit-input-note"),
    "A time limit can be at most 153722867280912 minutes."
  );
  form.set("task-form-budget-input", "1,000");
  form.set("task-form-time-limit-input", "0");
  assert_eq!(
    form.text("task-form-time-limit-input-note"),
    "The time limit must be at least one minute."
  );
  form.set("task-form-budget-input", "1.4m");
  form.set("task-form-time-limit-input", "half an hour");
  assert_eq!(
    form.text("task-form-budget-input-note"),
    "Use digits only, for example 1,400,000."
  );
  assert_eq!(
    form.text("task-form-time-limit-input-note"),
    "Enter whole minutes, such as 30."
  );
  assert_eq!(form.text("task-form-limits-value"), "Not valid");
  form.set("task-form-title-input", "Add a delivery log");
  assert!(
    form.reasons()[0].starts_with("To create, fix the run budget and fix the time limit."),
    "{:?}",
    form.reasons()
  );
}

#[test]
fn form_create_saves_a_draft_and_highlights_it_in_tasks() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.set("task-form-title-input", "Write the delivery docs");
  form.click("task-form-create");
  let highlighted = Route::Project(
    form.project,
    ProjectPage::Tasks(TaskList::default().with_highlight("KON-7".parse().expect("a key"))),
  );
  form.window.until("Tasks with the new task highlighted", |window| {
    window.navigation().route_now() == highlighted
  });
  let task = form.task("KON-7").expect("Create stored the task");
  assert_eq!(task.status(), TaskStatus::Draft);
  assert_eq!(task.details().title.as_str(), "Write the delivery docs");
  assert_eq!(
    task.details().limits,
    orchester_core::task::TaskLimits::NONE,
    "empty limits are no limits"
  );
}

#[test]
fn form_create_and_run_without_a_runner_queues_once_and_retries_the_same_run() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.fill("Add a delivery log", "Quick fix v1");
  assert_eq!(form.reasons(), vec![READY.to_owned()]);
  form.click("task-form-run");
  form.window.until("the run's refusal", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Queued · run not started")
  });
  assert_eq!(
    form.text("task-form-queued-note-text"),
    "Saved as KON-7. The fields stay locked while its run waits to start."
  );
  assert_eq!(
    form.reasons(),
    vec!["The runner is not connected. Try again once it is online.".to_owned()]
  );
  assert!(!form.exists("task-form-run"), "Create & run gave way to Retry run");
  let tasks = form.db().tasks().expect("tasks").len();
  let queued = form.task("KON-7").expect("the task is stored");
  assert_eq!(queued.status(), TaskStatus::Queued);
  form.click("task-form-retry");
  form.window.until("the retry's refusal", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Queued · run not started")
      && window.tree.get_element_by_id("task-form-retry").is_some()
  });
  assert_eq!(
    form.reasons(),
    vec!["The runner is not connected. Try again once it is online.".to_owned()]
  );
  assert_eq!(form.db().tasks().expect("tasks").len(), tasks, "no second task");
  assert!(
    form.db().runs_of_task(queued.id()).expect("runs").is_empty(),
    "no run was made"
  );
}

#[test]
fn form_save_changes_only_after_a_change() {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  assert_eq!(form.status(), "Draft saved locally");
  assert_eq!(form.text("task-form-crumbs-1"), "KON-1");
  assert_eq!(
    form.reasons(),
    vec![READY.to_owned()],
    "nothing to save: the draft can run"
  );
  assert!(form.exists("task-form-run"), "a draft can still Create & run");
  form.set("task-form-title-input", "A new title for KON-1");
  assert_eq!(form.status(), "Unsaved changes");
  form.click("task-form-save");
  form.window.until("the change is saved", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Draft saved locally")
  });
  let saved = form.task("KON-1").expect("KON-1");
  assert_eq!(saved.details().title.as_str(), "A new title for KON-1");
}

#[test]
fn form_discard_asks_in_place_and_restores_what_is_saved() {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  let saved = form.value("task-form-title-input");
  form.set("task-form-title-input", "Something else");
  form.click("task-form-discard");
  assert_eq!(
    form.text("task-form-discard-question"),
    "Discard your changes to KON-1?"
  );
  form.click("task-form-keep-editing");
  assert_eq!(form.value("task-form-title-input"), "Something else");
  form.click("task-form-discard");
  form.click("task-form-confirm-discard");
  assert_eq!(form.value("task-form-title-input"), saved);
  assert_eq!(form.status(), "Draft saved locally");
  assert!(!form.exists("task-form-discard-question"));
}

#[test]
fn form_leaving_with_unsaved_changes_keeps_them() {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  form.set("task-form-title-input", "Kept on this computer");
  let task = form.task("KON-1").expect("KON-1").id();
  let key = format!("orchester-app.task_form.kept.{}.{task}", form.project);
  form.window.until("the edits are kept", |window| {
    let workspace = window.launch.workspace().expect("the session started");
    workspace
      .ui_state(&key)
      .expect("the window state reads")
      .is_some_and(|kept| kept.contains("Kept on this computer"))
  });
  form.go(ProjectPage::Tasks(TaskList::default()));
  form.edit("KON-1");
  assert_eq!(form.value("task-form-title-input"), "Kept on this computer");
  assert!(
    form.status().starts_with("Unsaved changes · kept from "),
    "{}",
    form.status()
  );
  assert!(
    form.text("task-form-kept-note-text").starts_with("Your changes from "),
    "the note says where they came from"
  );
  assert_ne!(
    form.task("KON-1").expect("KON-1").details().title.as_str(),
    "Kept on this computer",
    "leaving saved nothing"
  );
}

#[test]
fn form_a_task_that_ran_has_no_create_and_run() {
  let mut form = Form::seeded("tasks");
  form.edit("KON-12");
  assert!(!form.exists("task-form-run"));
  assert_eq!(
    form.text("task-form-ran-note-text"),
    "KON-12 already ran. Changes apply to future runs; its runs keep the brief they started with."
  );
  assert_eq!(form.status(), "Saved");
  assert_eq!(
    form.reasons(),
    vec!["Nothing to save: no field has changed.".to_owned()]
  );
  let keys = if cfg!(target_os = "macos") { "⌘ S" } else { "Ctrl S" };
  assert_eq!(form.text("task-form-shortcut"), keys);
}

#[test]
fn form_without_published_pipelines_still_creates_a_draft() {
  let mut form = Form::bare();
  form.new_task();
  assert_eq!(form.text("task-form-pipeline-value"), "None published");
  form.open("task-form-pipeline");
  assert!(form.exists("task-form-pipeline-none"));
  assert!(form.exists("task-form-manage-pipelines"));
  form.click("task-form-pipeline");
  form.set("task-form-title-input", "Draft without a pipeline");
  let reasons = form.reasons();
  assert_eq!(reasons.len(), 1, "{reasons:?}");
  assert!(
    reasons[0].contains("no published pipeline in this project"),
    "{reasons:?}"
  );
  form.click("task-form-create");
  form.window.until("Tasks opens", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::Tasks(_))
    )
  });
}

#[test]
fn form_criteria_move_with_the_keyboard() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.click("task-form-add-criterion");
  form.click("task-form-add-criterion");
  form.set("task-form-criterion-1-input", "First");
  form.set("task-form-criterion-2-input", "Second");
  form
    .window
    .tree
    .get_element_by_id_mut("task-form-criterion-1-input")
    .expect("the first criterion")
    .focus();
  form.window.pass();
  key(&mut form, "ArrowDown", true);
  assert_eq!(form.value("task-form-criterion-1-input"), "Second");
  assert_eq!(form.value("task-form-criterion-2-input"), "First");
  key(&mut form, "Enter", false);
  assert!(
    form.exists("task-form-criterion-3-input"),
    "Enter adds a criterion after"
  );
  assert_eq!(form.value("task-form-criterion-3-input"), "");
  key(&mut form, "Backspace", false);
  assert!(
    !form.exists("task-form-criterion-3-input"),
    "Backspace removes an empty one"
  );
}

/// A criterion row is 8 over and under one line (`cScHP`, 33 high): its ×, a 24 square, shows on it without
/// making it taller, and a press where the pointer sees it removes it.
#[test]
fn form_criteria_rows_keep_their_height_with_the_remove_button() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.click("task-form-add-criterion");
  form.set("task-form-criterion-1-input", "First");
  form.click("task-form-add-criterion");
  form.set("task-form-criterion-2-input", "Second");
  let row = form.bounds("task-form-criterion-1");
  let remove = form.bounds("task-form-criterion-1-remove");
  assert!(row.height < remove.height + 16.0, "the row is {} high", row.height);
  assert!(
    remove.height > 0.0 && remove.y >= row.y && remove.y + remove.height <= row.y + row.height,
    "the × sits on its row: {remove:?} in {row:?}"
  );
  let input = form.bounds("task-form-criterion-1-input");
  assert!(
    input.x + input.width <= remove.x,
    "the text stops before the ×: {input:?}, {remove:?}"
  );
  let (x, y) = (remove.x + remove.width / 2.0, remove.y + remove.height / 2.0);
  let tree = &mut form.window.tree;
  tree.mouse_move(x, y);
  tree.mouse_down(x, y, MouseButton::Left);
  tree.mouse_up(x, y, MouseButton::Left);
  form.window.pass();
  assert_eq!(
    form.value("task-form-criterion-1-input"),
    "Second",
    "a press on the × removes its criterion"
  );
  assert!(!form.exists("task-form-criterion-2"), "one criterion is left");
}

fn focus(form: &mut Form, id: &str) {
  form
    .window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .focus();
  form.window.pass();
}

fn key(form: &mut Form, key: &str, alt: bool) {
  let tree = &mut form.window.tree;
  tree.key_down(key.to_owned(), key.to_owned(), false, false, alt);
  tree.key_up(key.to_owned(), key.to_owned(), false, false, alt);
  form.window.pass();
}
