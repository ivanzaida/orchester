//! New task and Edit task (`screens::task_form`) headless at 1440 × 900 on seeded Kontur projects whose
//! runner never starts: what each action needs is said in the design's words; the limits read back and
//! refuse only what is not a number, with no maximum; Create saves a draft and highlights it in Tasks;
//! Create & run without a runner leaves the task queued with Retry run, which never makes a second task or
//! run; Edit task saves only a change, discards in place, keeps unsaved changes when left, and offers no
//! Create & run for a task that ran. Needs `git` (the seed).

mod harness;
#[allow(
  dead_code,
  reason = "the seed's scenarios are shared with its own example; these tests use a part"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

use harness::Form;
use orchester_app::routes::{ProjectPage, Route, TaskList};
use orchester_core::{
  store::{RunStore, TaskStore},
  task::TaskStatus,
};

const RUN_REASON_EMPTY: &str = "Run unavailable: add a task title, describe the outcome, add an acceptance criterion, \
                                choose a pipeline and set a run budget and a time limit (runs need both for now).";

#[test]
fn form_new_task_says_what_each_action_needs() {
  let mut form = Form::seeded("live");
  form.new_task();
  assert_eq!(form.status(), "Not saved yet");
  assert_eq!(form.text("task-form-title"), "What would you like to build?");
  assert_eq!(
    form.reasons(),
    vec![
      "Create unavailable: add a task title.".to_owned(),
      RUN_REASON_EMPTY.to_owned()
    ]
  );
  assert!(
    form.exists("task-form-no-plan"),
    "the plan's place says to choose a pipeline"
  );
  form.set("task-form-title-input", "Add a delivery log");
  form.choose_pipeline("Quick fix v1");
  assert_eq!(
    form.reasons(),
    vec![
      "Run unavailable: describe the outcome, add an acceptance criterion and set a run budget and a time limit \
       (runs need both for now)."
        .to_owned()
    ]
  );
  assert!(form.exists("task-form-plan"), "the chosen pipeline's plan shows");
}

#[test]
fn form_limits_read_back_and_refuse_only_what_is_not_a_number() {
  let mut form = Form::seeded("live");
  form.new_task();
  assert_eq!(form.text("task-form-budget-input-read-back"), "No limit");
  assert_eq!(form.text("task-form-time-limit-input-read-back"), "No limit");
  form.set("task-form-budget-input", "1,400,000");
  form.set("task-form-time-limit-input", "1");
  assert_eq!(
    form.text("task-form-budget-input-read-back"),
    "= 1.4M tokens · input + output"
  );
  assert_eq!(
    form.text("task-form-time-limit-input-read-back"),
    "= 1 minute of active agent time"
  );
  // No maximum: a budget far above any project's old maximum is a budget.
  form.set("task-form-budget-input", "50,000,000,000");
  assert_eq!(
    form.text("task-form-budget-input-read-back"),
    "= 50B tokens · input + output"
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
  form.set("task-form-title-input", "Add a delivery log");
  assert_eq!(
    form.reasons()[0],
    "Create unavailable: fix the run budget and fix the time limit."
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
  assert_eq!(task.details().limits, None, "empty limits are no limits");
}

#[test]
fn form_create_and_run_without_a_runner_queues_once_and_retries_the_same_run() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.fill("Add a delivery log", "Quick fix v1");
  assert!(form.reasons().is_empty(), "nothing is missing: {:?}", form.reasons());
  form.click("task-form-run");
  form.window.until("the run's refusal", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Queued · run not started")
  });
  assert_eq!(
    form.text("task-form-description"),
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
      .texts("task-form-reasons")
      .is_some_and(|reasons| !reasons.is_empty())
      && window.tree.get_element_by_id("task-form-retry").is_some()
  });
  assert_eq!(form.status(), "Queued · run not started");
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
  assert_eq!(form.text("task-form-title"), "Edit task");
  assert_eq!(form.status(), "Draft saved locally");
  assert_eq!(
    form.reasons(),
    vec!["Save changes unavailable: nothing has changed.".to_owned()]
  );
  assert!(form.exists("task-form-run"), "a draft can still Create & run");
  form.set("task-form-title-input", "A new title for KON-1");
  assert_eq!(form.status(), "Unsaved changes");
  assert!(form.reasons().is_empty());
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
    form.text("task-form-bar-note"),
    "Create & run is not offered: this task already has a run."
  );
  assert_eq!(form.status(), "Saved");
}

#[test]
fn form_without_published_pipelines_still_creates_a_draft() {
  let mut form = Form::bare();
  form.new_task();
  assert_eq!(
    form.value("task-form-pipeline-input"),
    "No published pipelines in this project"
  );
  assert!(form.exists("task-form-open-pipelines"));
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

fn key(form: &mut Form, key: &str, alt: bool) {
  let tree = &mut form.window.tree;
  tree.key_down(key.to_owned(), key.to_owned(), false, false, alt);
  tree.key_up(key.to_owned(), key.to_owned(), false, false, alt);
  form.window.pass();
}
