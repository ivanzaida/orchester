//! Returning the work of a run that ended from its task page (the project task ORC-44 · Return a finished run to an
//! earlier step): *Return the work to an earlier step* at the top of the Summary of a succeeded and of a failed run,
//! with the agent steps the run executed and the note their agent reads; *Return to a step* in the header's *More
//! actions* opens the Summary at it and rings it, and at the decision of a run that waits for the person. A merged run
//! offers neither; one whose worktree was removed says why. No runner is connected here: the return waits, and says
//! why.

use std::path::Path;

use super::{OFFLINE, Page, checkout_page, key_of};

/// The step choice's options, the label of the note, and the lead under the card's title.
fn shows_the_choice(page: &mut Page) {
  assert_eq!(
    page.text("task-return-ended-title"),
    "Return the work to an earlier step"
  );
  let options = page.options("task-return-ended-step");
  assert!(options.contains(&"03 Implement".to_owned()), "{options:?}");
  assert!(
    page
      .text("task-return-ended-note-field-label")
      .contains("Note for Builder"),
    "the implementing step is chosen first"
  );
}

/// The element `id` is ringed as the card a navigation pointed at.
fn pointed(page: &mut Page, id: &str) -> bool {
  page
    .window
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class("pointed"))
}

#[test]
fn return_ended_a_succeeded_run_goes_back_with_a_note_from_its_summary_and_header() {
  let _checkout = checkout_page();
  let mut page = Page::seeded("decisions");
  page.open("ready for review, merges cleanly", "task-return-ended");
  shows_the_choice(&mut page);
  // A note is required, and the runner carries the return out: without one it waits, and says why.
  page.set("task-return-ended-note", "Show the newest delivery first.");
  assert!(page.locked("task-return-ended-button"));
  assert_eq!(page.text("task-return-ended-reason"), OFFLINE);

  assert!(!pointed(&mut page, "task-return-ended"));
  page.choose_more("task-return-to-step");
  page.window.until("the card rings", |window| {
    window
      .tree
      .get_element_by_id("task-return-ended")
      .is_some_and(|element| element.has_class("pointed"))
  });
}

#[test]
fn return_ended_a_failed_run_goes_back_and_a_removed_worktree_says_why() {
  let mut page = Page::seeded("task-states");
  page.open("failed", "task-return-ended");
  shows_the_choice(&mut page);
  assert!(page.in_more("task-return-to-step"));
  assert_eq!(page.text("task-return-ended-reason"), OFFLINE);

  let key = key_of(&page, "failed");
  remove_worktree_of(&page.repo, &key);
  // The card reads the worktree again once it is shown again.
  page.open("merged", "task-notice");
  page.open("failed", "task-return-ended");
  page.until_says("task-return-ended-reason", "worktree was removed");
  assert!(page.locked("task-return-ended-button"));
}

#[test]
fn return_ended_a_merged_run_and_a_running_one_offer_no_return() {
  let mut page = Page::seeded("task-states");
  page.open("merged", "task-notice");
  page
    .window
    .until("the run's facts", |window| window.texts("task-facts-title").is_some());
  assert!(!page.has("task-return-ended"));
  assert!(!page.in_more("task-return-to-step"));
  page.open("running", "task-event");
  assert!(!page.has("task-return-ended"));
  assert!(!page.in_more("task-return-to-step"));
}

#[test]
fn return_ended_the_header_points_at_a_waiting_runs_decision() {
  let mut page = Page::seeded("decisions");
  page.open("waits unverified after Review", "task-retry-decision");
  page.choose_more("task-return-to-step");
  page.window.until("the decision rings", |window| {
    window
      .tree
      .get_element_by_id("task-retry-decision")
      .is_some_and(|element| element.has_class("pointed"))
  });
  assert!(!page.has("task-return-ended"), "the decision returns its work");
}

/// Removes the folder of the worktree of the latest run of task `key`, as a person or a cleanup might.
fn remove_worktree_of(repo: &Path, key: &str) {
  use orchester_core::store::{RunStore, TaskStore};
  let db = orchester_store::ProjectDb::open(repo).expect("the database opens");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("its run")
    .expect("readable");
  let worktree = repo.join(".orchester/worktrees").join(run.id().to_string());
  // On Windows a file another process still reads (a git status, a virus scan of the fresh checkout) stays until it
  // is closed, so the removal is tried again for a while.
  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
  while let Err(error) = std::fs::remove_dir_all(&worktree) {
    assert!(
      std::time::Instant::now() < deadline,
      "the worktree's folder removed: {error}"
    );
    std::thread::sleep(std::time::Duration::from_millis(200));
  }
}
