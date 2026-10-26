//! *Don't merge automatically* (ORC-69) under *…*, a part of the task's organization: Create and Create & run save it
//! with the new task, Save changes turns it on and off, a newer version saved elsewhere keeps the person's choice, and
//! leaving the form keeps it with the other edits.

use orchester_app::routes::{ProjectPage, TaskList};
use orchester_core::task::TaskStatus;

use crate::{harness::Form, versions::until_value};

const SWITCH: &str = "task-form-no-automatic-merge";
const MORE: &str = "task-form-more";

/// Whether the switch shows on: its knob at the end of its track.
fn shows_on(form: &mut Form) -> bool {
  let mut bounds = |id: String| {
    form
      .window
      .tree
      .get_element_by_id_mut(&id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (track, knob) = (bounds(format!("{SWITCH}-mark")), bounds(format!("{SWITCH}-knob")));
  knob.x - track.x > track.width / 2.0 - knob.width / 2.0
}

fn until_shows(form: &mut Form, on: bool) {
  form.window.until("the switch shows the choice", |window| {
    let mut probe = |id: String| {
      window
        .tree
        .get_element_by_id_mut(&id)
        .and_then(|element| element.bounds())
    };
    match (probe(format!("{SWITCH}-mark")), probe(format!("{SWITCH}-knob"))) {
      (Some(track), Some(knob)) => (knob.x - track.x > track.width / 2.0 - knob.width / 2.0) == on,
      _ => false,
    }
  });
}

fn until_saved(form: &mut Form, status: &str) {
  form.window.until("the change is saved", |window| {
    window
      .texts("task-form-status-label")
      .is_some_and(|shown| shown.join("") == status)
  });
}

fn opted_out(form: &Form, key: &str) -> bool {
  form
    .task(key)
    .unwrap_or_else(|| panic!("{key} is stored"))
    .organization()
    .no_automatic_merge()
}

#[test]
fn form_merging_create_saves_it_and_save_changes_turns_it_off() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.open(MORE);
  assert_eq!(form.text(&format!("{SWITCH}-label")), "Don't merge automatically");
  assert!(!shows_on(&mut form), "off for a new task");
  form.set("task-form-title-input", "Keep it for the person");
  form.click(SWITCH);
  until_shows(&mut form, true);
  form.click("task-form-create");
  form.window.until("Create stored the task", |window| {
    window.tree.get_element_by_id("task-form-title-input").is_none()
  });
  assert!(opted_out(&form, "KON-7"), "Create saved it with the task");

  form.edit("KON-7");
  form.open(MORE);
  until_shows(&mut form, true);
  assert_eq!(form.status(), "Draft saved locally");
  form.click(SWITCH);
  until_shows(&mut form, false);
  assert_eq!(form.status(), "Unsaved changes", "a change of its own");
  form.click("task-form-save");
  until_saved(&mut form, "Draft saved locally");
  assert!(!opted_out(&form, "KON-7"), "Save changes turned it off");
}

#[test]
fn form_merging_create_and_run_saves_it_with_the_queued_task() {
  let mut form = Form::seeded("live");
  form.new_task();
  form.fill("Add a delivery log", "Quick fix v1");
  form.open(MORE);
  form.click(SWITCH);
  until_shows(&mut form, true);
  form.click("task-form-run");
  until_saved(&mut form, "Queued · run not started");
  let queued = form.task("KON-7").expect("the task is stored");
  assert_eq!(queued.status(), TaskStatus::Queued);
  assert!(queued.organization().no_automatic_merge(), "saved before it was queued");
}

#[test]
fn form_merging_a_newer_version_keeps_the_persons_choice() {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  assert!(!opted_out(&form, "KON-1"));
  form.open(MORE);
  form.click(SWITCH);
  until_shows(&mut form, true);
  form.change_elsewhere("KON-1", |details| details.brief = "Brief from automation.".to_owned());
  until_value(&mut form, "task-form-brief-input", "Brief from automation.");
  assert!(shows_on(&mut form), "the person's choice stays on the newer version");
  assert!(!form.exists("task-form-conflict-fields"), "nothing to choose");
  form.click("task-form-save");
  until_saved(&mut form, "Draft saved locally");
  let stored = form.task("KON-1").expect("KON-1");
  assert!(stored.organization().no_automatic_merge());
  assert_eq!(
    stored.details().brief,
    "Brief from automation.",
    "the newer version survives"
  );
}

#[test]
fn form_merging_leaving_keeps_the_choice_with_the_edits() {
  let mut form = Form::seeded("live");
  let task = form.task("KON-1").expect("KON-1").id();
  form.edit("KON-1");
  form.open(MORE);
  form.click(SWITCH);
  form.until_kept(Some(task), "the choice is kept", |kept| {
    kept.is_some_and(|kept| kept.contains("\"no_automatic_merge\":true"))
  });
  form.go(ProjectPage::Tasks(TaskList::default()));
  form.edit("KON-1");
  form.open(MORE);
  until_shows(&mut form, true);
  assert!(
    form.status().starts_with("Unsaved changes · kept from "),
    "{}",
    form.status()
  );
  assert!(!opted_out(&form, "KON-1"), "leaving saved nothing");
  form.click("task-form-save");
  form.until_kept(Some(task), "the save removes them", |kept| kept.is_none());
  assert!(opted_out(&form, "KON-1"), "the kept choice is saved");
}
