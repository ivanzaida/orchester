//! Edits and the version they are made on: a change made elsewhere is taken while the fields hold no
//! edits, and never overwritten under edits or by edits kept from an earlier visit; kept edits go away
//! once saved or discarded, and New task keeps and discards them too.

use orchester_app::routes::{ProjectPage, TaskList};

use crate::harness::Form;

const CHANGED_MEANWHILE: &str =
  "The task was changed since you opened it, so nothing was saved. Open it again to see the newer version.";

fn tasks(form: &mut Form) {
  form.go(ProjectPage::Tasks(TaskList::default()));
}

#[test]
fn form_a_change_made_elsewhere_is_taken_without_edits_and_never_overwritten_under_them() {
  let mut form = Form::seeded("live");
  form.edit("KON-1");
  form.change_elsewhere("KON-1", |details| details.brief = "Brief from automation.".to_owned());
  form.window.until("the form takes the newer version", |window| {
    window
      .tree
      .get_element_by_id_mut("task-form-brief-input")
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == "Brief from automation.")
  });
  assert!(!form.exists("task-form-newer"), "no notice without edits");

  form.set("task-form-title-input", "My own title");
  form.change_elsewhere("KON-1", |details| {
    details.brief = "Second brief from automation.".to_owned()
  });
  form.window.until("the notice", |window| {
    window.tree.get_element_by_id("task-form-newer").is_some()
  });
  assert_eq!(form.value("task-form-title-input"), "My own title", "the edits stay");
  assert_eq!(form.value("task-form-brief-input"), "Brief from automation.");
  form.click("task-form-save");
  form.window.until("the save's refusal", |window| {
    window
      .texts("task-form-reasons")
      .is_some_and(|reasons| reasons.iter().any(|reason| reason == CHANGED_MEANWHILE))
  });
  let stored = form.task("KON-1").expect("KON-1");
  assert_eq!(
    stored.details().brief,
    "Second brief from automation.",
    "the change made elsewhere survives"
  );
  assert_ne!(stored.details().title.as_str(), "My own title");

  form.click("task-form-reload");
  assert_eq!(form.value("task-form-brief-input"), "Second brief from automation.");
  assert_eq!(form.status(), "Draft saved locally");
  assert!(!form.exists("task-form-newer"));
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
  form.change_elsewhere("KON-1", |details| details.brief = "Changed while away.".to_owned());
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
          .any(|task| task.details().brief == "Changed while away.")
      })
  });
  form.edit("KON-1");
  assert_eq!(form.value("task-form-title-input"), "Kept title");
  assert!(
    form.status().starts_with("Unsaved changes · kept from "),
    "{}",
    form.status()
  );
  form.window.until("the form says the task changed", |window| {
    window.tree.get_element_by_id("task-form-newer").is_some()
  });
  assert_eq!(form.value("task-form-title-input"), "Kept title", "the kept edits stay");
  form.click("task-form-save");
  form.window.until("the save's refusal", |window| {
    window
      .texts("task-form-reasons")
      .is_some_and(|reasons| reasons.iter().any(|reason| reason == CHANGED_MEANWHILE))
  });
  let stored = form.task("KON-1").expect("KON-1");
  assert_eq!(stored.details().brief, "Changed while away.");
}

#[test]
fn form_kept_edits_go_away_once_saved_or_discarded() {
  let mut form = Form::seeded("live");
  let task = form.task("KON-1").expect("KON-1").id();
  form.edit("KON-1");
  form.set("task-form-title-input", "Saved title");
  form.until_kept(Some(task), "the edits are kept", |kept| kept.is_some());
  form.click("task-form-save");
  form.until_kept(Some(task), "a save removes them", |kept| kept.is_none());

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
  assert_eq!(
    form.text("task-form-criteria-description"),
    "Reviewers check the result against each one."
  );
}
