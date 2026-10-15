//! Removing a run's worktrees from the task page when the run changed since the page read it, and
//! an interrupted run whose worktree folder is gone (ORC-63).

mod support;

use orchester_core::{
  integration::{CleanupError, Leftovers},
  run::InterruptCause,
  store::WorktreeStore,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  detail::TaskDetail,
  integration::{IntegrationProblem, remove_in},
};
use support::{Fixture, at};

#[test]
fn worktree_removal_is_refused_for_a_run_continued_since_the_page_read_it() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let events = run.interrupt(InterruptCause::QuitNow, at(30)).expect("interrupt");
  fixture.record(&run, &events);
  // What the page read; another window then continues the run.
  let read = run.clone();
  let events = run.continue_run(at(40)).expect("continue");
  fixture.record(&run, &events);

  let refused = remove_in(
    fixture.repo.path(),
    &Programs::processes(),
    &read,
    &Leftovers::default(),
  );
  assert_eq!(refused, Err(IntegrationProblem::Cleanup(CleanupError::LiveWork)));
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert_eq!(
    db.worktree_removal(run.id()).expect("read"),
    None,
    "nothing is recorded"
  );
}

#[test]
fn worktree_removal_is_refused_for_a_run_that_worked_again_in_another_worktree_since_the_page_read_it() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  // What the page read; another window then asks the run for a follow-up, which works in a new worktree
  // and is interrupted: the run has no live work, but more worktrees than the page listed.
  let read = run.clone();
  let step = run.snapshot().steps()[0].key().clone();
  let events = run.follow_up(&step, at(40)).expect("follow up");
  fixture.record(&run, &events);
  let events = run.interrupt(InterruptCause::QuitNow, at(50)).expect("interrupt");
  fixture.record(&run, &events);

  let refused = remove_in(
    fixture.repo.path(),
    &Programs::processes(),
    &read,
    &Leftovers::default(),
  );
  assert_eq!(refused, Err(IntegrationProblem::RunChanged));
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert_eq!(
    db.worktree_removal(run.id()).expect("read"),
    None,
    "nothing is recorded"
  );
}

#[test]
fn worktree_removal_is_recorded_once_an_interrupted_run_is_found_without_its_folder() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let events = run.interrupt(InterruptCause::QuitNow, at(30)).expect("interrupt");
  fixture.record(&run, &events);

  // The fixture's run never had a worktree folder: the page records it as removed.
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task page reads");
  let latest = detail.run.as_ref().expect("the run");
  assert!(latest.worktree_removed.is_some());
  assert!(detail.history[0].removed_at.is_some());
}
