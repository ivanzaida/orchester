//! Whether each pipeline is ready on this computer, in the Pipeline menu and in the action bar (`R2` `P6aAoZ` for a
//! team project, `L10` `PMi6Q` for a local one): Create & run is off with the reason while the chosen pipeline is not
//! ready, and Create still saves the task.

use std::{fs, thread, time::Duration};

use orchester_core::{store::TaskStore, task::TaskStatus};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::outside::Outside;

use crate::harness::Form;

const WAIT: Duration = Duration::from_secs(10);

/// New task with its Pipeline menu open and every pipeline's readiness read.
fn menu(scenario: &str) -> Form {
  let mut form = Form::seeded(scenario);
  form.new_task();
  form.open("task-form-pipeline");
  form.window.until_within(WAIT, "the pipelines' readiness", |window| {
    window.texts("task-form-pipeline-option-0-status").is_some()
  });
  form
}

/// Each pipeline of the menu with the words after its dot.
fn statuses(form: &mut Form) -> Vec<(String, String)> {
  let mut rows = Vec::new();
  for index in 0.. {
    let id = format!("task-form-pipeline-option-{index}");
    let Some(title) = form.window.texts(&format!("{id}-title")) else {
      break;
    };
    let status = form.window.texts(&format!("{id}-status")).unwrap_or_default();
    rows.push((title.join(""), status.join("")));
  }
  rows
}

fn status_of(rows: &[(String, String)], pipeline: &str) -> String {
  rows
    .iter()
    .find(|(title, _)| title.starts_with(pipeline))
    .unwrap_or_else(|| panic!("the menu offers {pipeline}"))
    .1
    .clone()
}

fn tasks(form: &Form) -> usize {
  form.db().tasks().expect("tasks").len()
}

#[test]
fn form_readiness_a_local_menu_says_ready_or_how_many_things_are_missing() {
  let mut form = menu("picker-readiness");
  let rows = statuses(&mut form);
  assert_eq!(status_of(&rows, "Feature delivery"), "Ready");
  assert_eq!(status_of(&rows, "Quick fix"), "Ready");
  assert_eq!(status_of(&rows, "Design pass"), "2 things missing");
  assert_eq!(status_of(&rows, "Review pass"), "1 thing missing");
}

#[test]
fn form_readiness_a_team_menu_says_here() {
  let mut form = menu("picker-readiness-team");
  let rows = statuses(&mut form);
  assert_eq!(status_of(&rows, "Team delivery"), "2 things missing here");
  assert_eq!(status_of(&rows, "Team quick fix"), "Ready here");
  form.fill("Add a team delivery", "Team delivery");
  assert_eq!(
    form.reasons(),
    vec!["Team delivery v1 is not ready on this computer · 2 things missing".to_owned()]
  );
}

#[test]
fn form_readiness_a_pipeline_not_ready_turns_off_create_and_run_with_the_reason_and_create_still_saves() {
  let mut form = menu("picker-readiness");
  form.click("task-form-pipeline-option-0");
  form.fill("Add a design pass", "Design pass");
  assert_eq!(
    form.reasons(),
    vec!["Design pass v1 is not ready · 2 things missing".to_owned()]
  );
  assert_eq!(form.text("task-form-reason-link-text"), "Show");
  let before = tasks(&form);
  form.click("task-form-run");
  assert_eq!(tasks(&form), before, "Create & run is off");
  form.click("task-form-create");
  let repo = form.repo.clone();
  form.window.until("the task is saved", |_| {
    ProjectDb::open(&repo)
      .expect("the database")
      .tasks()
      .expect("tasks")
      .len()
      == before + 1
  });
  let saved = form.db().tasks().expect("tasks").into_iter().last().expect("the task");
  assert_eq!(saved.details().title.as_str(), "Add a design pass");
  assert!(saved.details().pipeline.is_some(), "Create keeps the pipeline");
}

#[test]
fn form_readiness_a_ready_pipeline_leaves_create_and_run_on() {
  let mut form = menu("picker-readiness");
  form.fill("Add a delivery log", "Quick fix");
  assert!(
    form.reasons().iter().all(|reason| !reason.contains("not ready")),
    "{:?}",
    form.reasons()
  );
}

/// New task filled with Quick fix, ready to run once its readiness is read.
fn ready_to_run(scenario: &str) -> Form {
  let mut form = Form::seeded(scenario);
  form.new_task();
  form.fill("Add a delivery log", "Quick fix");
  until_reason(&mut form, "ready to run", |reason| reason.starts_with("Ready to run"));
  form
}

fn until_reason(form: &mut Form, what: &str, done: impl Fn(&str) -> bool) {
  form.window.until_within(WAIT, what, |window| {
    window
      .texts("task-form-reason-text")
      .is_some_and(|texts| done(&texts.join("")))
  });
}

/// Holds the reads and tells the form that a connection changed: its readiness is read again, and waits.
fn read_again_held(form: &mut Form) {
  let services = form.window.launch.services().expect("the session started").clone();
  services.hold.set(true);
  services.changes.record(form.project, Outside::Connections);
  form.window.pass();
}

fn release(form: &mut Form) {
  form
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .set(false);
}

/// Presses Create & run and lets the window settle: nothing may come of it.
fn press_run(form: &mut Form) {
  form.click("task-form-run");
  for _ in 0..30 {
    form.window.pass();
    thread::sleep(Duration::from_millis(10));
  }
}

#[test]
fn form_readiness_create_and_run_waits_while_the_readiness_is_read() {
  let mut form = ready_to_run("picker-readiness");
  read_again_held(&mut form);
  until_reason(&mut form, "the reading reason", |reason| {
    reason == "Reading whether Quick fix v2 is ready…"
  });
  let before = tasks(&form);
  press_run(&mut form);
  assert_eq!(tasks(&form), before, "Create & run is off while the readiness is read");
  release(&mut form);
  until_reason(&mut form, "ready to run again", |reason| {
    reason.starts_with("Ready to run")
  });
}

#[test]
fn form_readiness_an_answer_for_other_inputs_is_not_shown() {
  let mut form = menu("picker-readiness");
  assert_eq!(status_of(&statuses(&mut form), "Design pass"), "2 things missing");
  read_again_held(&mut form);
  form.window.until_within(WAIT, "the old answer to go", |window| {
    window.texts("task-form-pipeline-option-0-status").is_none()
  });
  assert!(
    statuses(&mut form).iter().all(|(_, status)| status.is_empty()),
    "no pipeline is called ready or not while its readiness is read: {:?}",
    statuses(&mut form)
  );
  release(&mut form);
  form.window.until_within(WAIT, "the new answer", |window| {
    window.texts("task-form-pipeline-option-0-status").is_some()
  });
  assert_eq!(status_of(&statuses(&mut form), "Design pass"), "2 things missing");
}

/// Moves the project's `.orchester` folder to `to`, which the readiness read cannot open its database without.
fn move_data(form: &Form, (from, to): (&str, &str)) {
  let (from, to) = (form.repo.join(from), form.repo.join(to));
  let deadline = std::time::Instant::now() + WAIT;
  while let Err(error) = fs::rename(&from, &to) {
    assert!(
      std::time::Instant::now() < deadline,
      "cannot move {}: {error}",
      from.display()
    );
    thread::sleep(Duration::from_millis(20));
  }
}

#[test]
fn form_readiness_a_read_that_failed_is_said_and_holds_create_and_run_until_retry_reads_it() {
  let mut form = ready_to_run("picker-readiness");
  move_data(&form, (".orchester", ".orchester-away"));
  let services = form.window.launch.services().expect("the session started").clone();
  services.changes.record(form.project, Outside::Connections);
  until_reason(&mut form, "the failed read", |reason| {
    reason == "Couldn't tell whether Quick fix v2 is ready"
  });
  assert_eq!(form.text("task-form-reason-link-text"), "Retry");
  press_run(&mut form);
  assert!(!form.exists("task-form-retry"), "Create & run started nothing");
  form.open("task-form-pipeline");
  let rows = statuses(&mut form);
  assert_eq!(status_of(&rows, "Quick fix"), "Couldn't tell");
  assert_eq!(status_of(&rows, "Design pass"), "Couldn't tell");
  move_data(&form, (".orchester-away", ".orchester"));
  form.click("task-form-reason-link");
  until_reason(&mut form, "ready to run once it is read again", |reason| {
    reason.starts_with("Ready to run")
  });
}

#[test]
fn form_readiness_edit_task_waits_and_refuses_like_new_task() {
  let mut form = menu("picker-readiness");
  form.click("task-form-pipeline-option-0");
  form.fill("Add a design pass", "Design pass");
  form.click("task-form-create");
  let repo = form.repo.clone();
  form.window.until("the draft is saved", |_| {
    !ProjectDb::open(&repo)
      .expect("the database")
      .tasks()
      .expect("tasks")
      .is_empty()
  });
  let key = form
    .db()
    .tasks()
    .expect("tasks")
    .into_iter()
    .last()
    .expect("the task")
    .key()
    .to_string();
  form.window.until_within(WAIT, "Tasks after Create", |window| {
    window.texts("screen-tasks").is_some()
  });
  form.edit(&key);
  until_reason(&mut form, "the missing things", |reason| {
    reason == "Design pass v1 is not ready · 2 things missing"
  });
  read_again_held(&mut form);
  until_reason(&mut form, "the reading reason", |reason| {
    reason == "Reading whether Design pass v1 is ready…"
  });
  press_run(&mut form);
  let task = form.task(&key).expect("the task is stored");
  assert_eq!(task.status(), TaskStatus::Draft, "Edit task's Create & run is off too");
  release(&mut form);
  until_reason(&mut form, "the missing things again", |reason| {
    reason == "Design pass v1 is not ready · 2 things missing"
  });
}
