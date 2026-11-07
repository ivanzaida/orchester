//! Whether each pipeline is ready on this computer, in the Pipeline menu and in the action bar (`R2` `P6aAoZ` for a
//! team project, `L10` `PMi6Q` for a local one): Create & run is off with the reason while the chosen pipeline is not
//! ready, and Create still saves the task.

use std::time::Duration;

use orchester_core::store::TaskStore;
use orchester_store::ProjectDb;

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
  assert_eq!(form.text("task-form-reason-show-text"), "Show");
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
