//! The property chips' settings that today's form did not have: what the task's runs deliver (ORC-93) and its
//! priority, saved with the task's organization through the same task write as the other fields; and the keys: Ctrl
//! Enter asks for Create & run, and while the run still needs a pipeline the Pipeline chip says its name in `warning`.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{resource::Deliverable, task::organization::Priority};

use crate::harness::Form;

fn until_tasks(form: &mut Form) {
  form.window.until("Tasks opens", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::Tasks(_))
    )
  });
}

#[test]
fn form_delivers_outside_is_kept_and_the_default_leaves_it_unset() {
  let mut form = Form::seeded("live");
  form.new_task();
  assert_eq!(form.text("task-form-delivers-value"), "As the pipeline says");
  form.set("task-form-title-input", "Draw the delivery log screen");
  form.open("task-form-delivers");
  assert_eq!(form.text("task-form-delivers-outside-title"), "Outside the repository");
  form.click("task-form-delivers-outside");
  assert_eq!(form.text("task-form-delivers-value"), "Outside the repo");
  form.click("task-form-create");
  until_tasks(&mut form);
  let outside = form.task("KON-7").expect("Create stored the task");
  assert_eq!(outside.organization().delivers(), Some(Deliverable::Outside));

  form.new_task();
  form.set("task-form-title-input", "Write the delivery docs");
  form.click("task-form-create");
  until_tasks(&mut form);
  let plain = form.task("KON-8").expect("Create stored the task");
  assert_eq!(plain.organization().delivers(), None, "as the pipeline says");
}

#[test]
fn form_delivers_is_changed_on_edit_and_the_ready_line_says_what_happens() {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  assert_eq!(form.reasons(), vec![crate::READY.to_owned()]);
  form.open("task-form-delivers");
  form.click("task-form-delivers-outside");
  assert_eq!(form.status(), "Unsaved changes");
  assert_eq!(
    form.reasons(),
    vec!["Ready to run · the result is its report and the MCP changes; no branch to merge.".to_owned()]
  );
  form.click("task-form-save");
  form.window.until("the change is saved", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Draft saved locally")
  });
  let saved = form.task("KON-1").expect("KON-1");
  assert_eq!(saved.organization().delivers(), Some(Deliverable::Outside));
  form.open("task-form-delivers");
  form.click("task-form-delivers-pipeline");
  form.click("task-form-save");
  form.window.until("the default is saved", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Draft saved locally")
  });
  assert_eq!(form.task("KON-1").expect("KON-1").organization().delivers(), None);
}

#[test]
fn form_priority_is_saved_with_the_task() {
  let mut form = Form::seeded("live");
  form.new_task();
  assert_eq!(form.text("task-form-priority-value"), "None");
  form.set("task-form-title-input", "Write the delivery docs");
  form.open("task-form-priority");
  form.click("task-form-priority-High");
  assert_eq!(form.text("task-form-priority-value"), "High");
  form.click("task-form-create");
  until_tasks(&mut form);
  let task = form.task("KON-7").expect("Create stored the task");
  assert_eq!(task.organization().priority(), Priority::High);
}

/// Ctrl and Enter, as a person presses them, wherever the focus is.
// lurq's headless keys carry no Cmd, which macOS asks for instead of Ctrl.
#[cfg(not(target_os = "macos"))]
fn ctrl_enter(form: &mut Form) {
  let tree = &mut form.window.tree;
  tree.key_down("Enter".to_owned(), "Enter".to_owned(), false, true, false);
  tree.key_up("Enter".to_owned(), "Enter".to_owned(), false, true, false);
  form.window.pass();
}

#[cfg(not(target_os = "macos"))]
#[test]
fn form_ctrl_enter_asks_for_create_and_run_and_says_what_holds_it_back() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.set("task-form-title-input", "Add a delivery log");
  form.set("task-form-brief-input", "Record every delivery attempt.");
  form.click("task-form-add-criterion");
  form.set("task-form-criterion-1-input", "Every attempt is stored.");
  ctrl_enter(&mut form);
  assert!(form.task("KON-7").is_none(), "nothing runs without a pipeline");
  assert_eq!(form.reasons(), vec!["To run, choose a pipeline.".to_owned()]);
  assert!(
    form.exists("task-form-pipeline-label"),
    "the empty chip names what is missing"
  );
  form.choose_pipeline("Quick fix v1");
  ctrl_enter(&mut form);
  form.window.until("the run's refusal", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Queued · run not started")
  });
  assert!(form.task("KON-7").is_some(), "Ctrl Enter created and queued the task");
}
