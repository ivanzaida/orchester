//! Fields both versions changed (`Edit task · conflict · newer version` `PenUu`): each shows the person's
//! version and the newer one to choose from; saving the chosen versions stores them on the newer version,
//! a change saved meanwhile is met again and never overwritten, and Back to editing keeps the edits.

use orchester_core::name::Name;

use crate::harness::Form;

/// KON-1 edited (title and brief) while another version changed both.
fn conflicted() -> Form {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  form.set("task-form-title-input", "My title");
  form.set("task-form-brief-input", "My brief.");
  form.change_elsewhere("KON-1", |details| {
    details.title = Name::new("Their title").expect("a title");
    details.brief = "Their brief.".to_owned();
  });
  form.window.until("the fields to choose", |window| {
    window.tree.get_element_by_id("task-form-conflict-fields").is_some()
  });
  form
}

fn until_saved(form: &mut Form) {
  form.window.until("the chosen versions are saved", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Draft saved locally")
  });
}

#[test]
fn form_conflict_saves_the_version_chosen_for_each_field() {
  let mut form = conflicted();
  assert_eq!(form.status(), "Newer version saved");
  assert_eq!(
    form.text("task-form-conflict-notice-title"),
    "A newer version of KON-1 was saved while you were editing"
  );
  assert_eq!(
    form.reasons(),
    vec!["Save unavailable: choose a version for Task title and Describe the outcome.".to_owned()]
  );
  assert!(!form.exists("task-form-run"), "no Create & run while choosing");
  form.click("task-form-conflict-title-mine");
  form.click("task-form-conflict-brief-theirs");
  assert!(form.reasons().is_empty(), "{:?}", form.reasons());
  form.click("task-form-save-chosen");
  until_saved(&mut form);
  let stored = form.task("KON-1").expect("KON-1");
  assert_eq!(stored.details().title.as_str(), "My title");
  assert_eq!(stored.details().brief, "Their brief.");
  assert!(!form.exists("task-form-conflict-fields"));
  assert_eq!(form.value("task-form-title-input"), "My title");
  assert_eq!(form.value("task-form-brief-input"), "Their brief.");
}

#[test]
fn form_conflict_meets_a_change_saved_during_the_choice() {
  let mut form = conflicted();
  form.click("task-form-conflict-title-mine");
  form.click("task-form-conflict-brief-theirs");
  form.change_elsewhere("KON-1", |details| {
    details.title = Name::new("A third title").expect("a title")
  });
  form.click("task-form-save-chosen");
  form.window.until("the third change to choose from", |window| {
    window
      .texts("task-form-conflict-title-theirs-value")
      .is_some_and(|value| value.join("") == "A third title")
  });
  assert_eq!(
    form.task("KON-1").expect("KON-1").details().title.as_str(),
    "A third title",
    "the change saved meanwhile is not overwritten"
  );
  assert!(
    form.reasons().iter().any(|reason| reason.contains("Task title")),
    "the title is to choose again: {:?}",
    form.reasons()
  );
}

#[test]
fn form_back_to_editing_keeps_my_edits() {
  let mut form = conflicted();
  form.click("task-form-back-to-editing");
  assert!(!form.exists("task-form-conflict-fields"));
  assert_eq!(form.value("task-form-title-input"), "My title");
  assert_eq!(form.value("task-form-brief-input"), "My brief.");
  assert_eq!(form.status(), "Newer version saved");
  let stored = form.task("KON-1").expect("KON-1");
  assert_eq!(stored.details().title.as_str(), "Their title");
  // Saving meets the newer version again: the fields are to choose once more.
  form.click("task-form-save");
  form.window.until("the fields to choose again", |window| {
    window.tree.get_element_by_id("task-form-conflict-fields").is_some()
  });
  assert_eq!(
    form.task("KON-1").expect("KON-1").details().title.as_str(),
    "Their title"
  );
}
