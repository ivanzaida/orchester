//! Edits and the version they are made on: a change made elsewhere is taken while the fields hold no
//! edits and put under them when it changed other fields, also under edits kept from an earlier visit;
//! kept edits go away once saved or discarded, and New task keeps and discards them too. Fields both
//! versions changed are `conflicts.rs`.

use orchester_app::routes::{ProjectPage, TaskList};

use crate::harness::Form;

fn tasks(form: &mut Form) {
  form.go(ProjectPage::Tasks(TaskList::default()));
}

/// Waits until the input `id` holds `value`.
pub fn until_value(form: &mut Form, id: &str, value: &str) {
  form.window.until(&format!("{id} holds {value}"), |window| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == value)
  });
}

#[test]
fn form_a_change_made_elsewhere_is_taken_without_edits_and_put_under_them() {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  form.change_elsewhere("KON-1", |details| details.brief = "Brief from automation.".to_owned());
  until_value(&mut form, "task-form-brief-input", "Brief from automation.");

  form.set("task-form-title-input", "My own title");
  form.change_elsewhere("KON-1", |details| {
    details.brief = "Second brief from automation.".to_owned()
  });
  until_value(&mut form, "task-form-brief-input", "Second brief from automation.");
  assert_eq!(form.value("task-form-title-input"), "My own title", "the edits stay");
  assert!(
    !form.exists("task-form-conflict-fields"),
    "nothing to choose: they changed other fields"
  );
  assert_eq!(form.status(), "Unsaved changes");
  form.click("task-form-save");
  form.window.until("the change is saved", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Draft saved locally")
  });
  let stored = form.task("KON-1").expect("KON-1");
  assert_eq!(stored.details().title.as_str(), "My own title");
  assert_eq!(
    stored.details().brief,
    "Second brief from automation.",
    "the change made elsewhere survives"
  );
}

#[test]
fn form_kept_edits_made_on_an_older_version_meet_the_newer_one() {
  let mut form = Form::seeded("live");
  let task = form.task("KON-1").expect("KON-1").id();
  form.edit("KON-1");
  form.set("task-form-title-input", "Kept title");
  form.until_kept(Some(task), "the edits are kept", |kept| {
    kept.is_some_and(|kept| kept.contains("Kept title"))
  });
  tasks(&mut form);
  form.change_elsewhere("KON-1", |details| {
    details.title = orchester_core::name::Name::new("Title saved while away").expect("a title");
  });
  // The form opens on the newer version, so the kept edits are filled in on the older one they were
  // made on.
  let project = form.project;
  form.window.until("the tasks are read again", |window| {
    window
      .projects()
      .feed(project)
      .tasks
      .get_untracked()
      .ready()
      .is_some_and(|tasks| {
        tasks
          .tasks
          .iter()
          .any(|task| task.details().title.as_str() == "Title saved while away")
      })
  });
  form.edit("KON-1");
  assert_eq!(form.status(), "Newer version saved");
  assert!(
    form.exists("task-form-conflict-title"),
    "the title is the person's to choose"
  );
  assert_eq!(form.text("task-form-conflict-title-mine-value"), "Kept title");
  assert_eq!(
    form.text("task-form-conflict-title-theirs-value"),
    "Title saved while away"
  );
  let stored = form.task("KON-1").expect("KON-1");
  assert_eq!(
    stored.details().title.as_str(),
    "Title saved while away",
    "nothing was overwritten"
  );
}

#[test]
fn form_kept_edits_go_away_once_saved_or_discarded() {
  let mut form = Form::seeded("live");
  let task = form.task("KON-1").expect("KON-1").id();
  form.edit("KON-1");
  form.set("task-form-title-input", "Saved title");
  form.until_kept(Some(task), "the edits are kept", |kept| kept.is_some());
  let tab = form.window.navigation().tabs_now().active().id();
  let dot = format!("tab-{tab}-unsaved");
  assert!(form.exists(&dot), "the tab marks the unsaved edits");
  form.click("task-form-save");
  form.until_kept(Some(task), "a save removes them", |kept| kept.is_none());
  assert!(!form.exists(&dot), "a saved form's tab has no mark");

  form.set("task-form-title-input", "Discarded title");
  form.until_kept(Some(task), "the edits are kept", |kept| kept.is_some());
  form.click("task-form-discard");
  form.click("task-form-confirm-discard");
  form.until_kept(Some(task), "a discard removes them", |kept| kept.is_none());
  assert_eq!(form.value("task-form-title-input"), "Saved title");
}

#[test]
fn form_new_task_keeps_its_edits_and_discards_them() {
  let mut form = Form::seeded("live");
  form.new_task();
  assert!(!form.exists("task-form-discard"), "a fresh form offers no Discard");
  form.set("task-form-title-input", "Half-written task");
  form.until_kept(None, "the edits are kept", |kept| {
    kept.is_some_and(|kept| kept.contains("Half-written task"))
  });
  tasks(&mut form);
  form.new_task();
  assert_eq!(form.value("task-form-title-input"), "Half-written task");
  assert!(
    form.status().starts_with("Unsaved changes · kept from "),
    "{}",
    form.status()
  );
  form.click("task-form-discard");
  assert_eq!(form.text("task-form-discard-question"), "Discard your unsaved task?");
  form.click("task-form-confirm-discard");
  assert_eq!(form.value("task-form-title-input"), "");
  assert_eq!(form.status(), "Not saved yet");
  form.until_kept(None, "a discard removes them", |kept| kept.is_none());
}

#[test]
fn form_retry_run_asks_the_runner_again() {
  let mut form = Form::seeded("tasks");
  form.edit("KON-18");
  assert_eq!(form.status(), "Queued · run not started");
  form.click("task-form-retry");
  assert_eq!(form.status(), "Starting the run…", "the retry is a request of its own");
  form.window.until("the retry's answer", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|status| status.join("") == "Queued · run not started")
  });
  assert!(!form.reasons().is_empty(), "the runner's answer is said");
}

#[test]
fn form_backspace_removes_the_only_criterion() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.click("task-form-add-criterion");
  form
    .window
    .tree
    .get_element_by_id_mut("task-form-criterion-1-input")
    .expect("the criterion")
    .focus();
  form.window.pass();
  let tree = &mut form.window.tree;
  tree.key_down("Backspace".to_owned(), "Backspace".to_owned(), false, false, false);
  tree.key_up("Backspace".to_owned(), "Backspace".to_owned(), false, false, false);
  form.window.pass();
  assert!(!form.exists("task-form-criterion-1"));
  assert!(!form.exists("task-form-criteria-count"), "no count without criteria");
  assert_eq!(
    form.text("task-form-criteria-hint"),
    "· reviewers check the result against each one"
  );
}

#[test]
fn form_a_queued_task_removes_edits_kept_for_it() {
  let mut form = Form::seeded("tasks");
  let task = form.task("KON-18").expect("KON-18").id();
  form.keep_earlier("KON-18", "Edits that cannot be saved");
  form.until_kept(Some(task), "the earlier edits are there", |kept| kept.is_some());
  form.edit("KON-18");
  assert_eq!(form.status(), "Queued · run not started");
  form.until_kept(Some(task), "opening the queued task removes them", |kept| {
    kept.is_none()
  });
}

#[test]
fn form_a_save_removes_edits_an_earlier_visit_kept() {
  let mut form = Form::seeded("live");
  let task = form.task("KON-1").expect("KON-1").id();
  form.keep_earlier("KON-1", "Title kept by an earlier visit");
  form.edit("KON-1");
  assert_eq!(form.value("task-form-title-input"), "Title kept by an earlier visit");
  assert!(
    form.status().starts_with("Unsaved changes · kept from "),
    "{}",
    form.status()
  );
  form.click("task-form-save");
  form.until_kept(Some(task), "the save removes them", |kept| kept.is_none());
  assert_eq!(
    form.task("KON-1").expect("KON-1").details().title.as_str(),
    "Title kept by an earlier visit"
  );
}
