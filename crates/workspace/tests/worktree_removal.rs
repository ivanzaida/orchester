//! Removing a run's worktrees from the task page when the run changed since the page read it, and
//! an interrupted run whose worktree folder is gone (ORC-63).

mod support;

use std::sync::{Arc, mpsc};

use orchester_core::{
  integration::{CleanupError, Leftovers},
  run::InterruptCause,
  store::WorktreeStore,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  activity::Activity,
  tasks::{
    detail::TaskDetail,
    integration::{IntegrationProblem, remove_in},
    integration_jobs::{IntegrationJobs, RemoveArgs, Remover},
  },
  workspace::OpenProject,
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

#[test]
fn worktree_removal_automation_asked_for_tells_it_the_run_changed() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  fixture.succeed(&mut run, at(30));
  // What the tool read; the run then works again in a new worktree and is interrupted.
  let read = run.clone();
  let step = run.snapshot().steps()[0].key().clone();
  let events = run.follow_up(&step, at(40)).expect("follow up");
  fixture.record(&run, &events);
  let events = run.interrupt(InterruptCause::QuitNow, at(50)).expect("interrupt");
  fixture.record(&run, &events);

  let open = OpenProject {
    project: fixture.db.project().clone(),
    repo_root: fixture.repo.path().to_path_buf(),
  };
  let (answer, outcome) = mpsc::channel();
  IntegrationJobs::default().of(run.id()).start_remove(RemoveArgs {
    open,
    task,
    key: "ORC-1".into(),
    number: run.number().get(),
    run: read,
    confirmed: Leftovers::default(),
    programs: Programs::processes(),
    activity: Activity::new(),
    separator: ", ".into(),
    changed: Arc::new(|| {}),
    by: Remover::Automation(answer),
  });
  let refused = outcome.recv().expect("an answer").expect_err("refused");
  assert!(refused.run_changed, "{refused:?}");
  assert_eq!(refused.message.key, "runs.worktree.failure.runChanged");
}
