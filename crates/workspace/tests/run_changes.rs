//! Each run's change count beside its project's (`Runners::run_changes_of`): a run's changes and transcript lines
//! move its own count and the project's, never another run's; a change that names no run moves every run's.

use std::path::{Path, PathBuf};

use orchester_core::id::RunId;
use orchester_ipc::SessionEnd;
use orchester_workspace::{activity::Activity, runner::Runners};
use tokio::sync::watch;

fn runners(runtime: &tokio::runtime::Runtime) -> Runners {
  Runners::new(
    PathBuf::from("no-runner"),
    runtime.handle().clone(),
    Activity::new(),
    SessionEnd::Clean,
  )
}

/// Whether `count` moved since it was last seen; marks it seen.
fn moved(count: &mut watch::Receiver<u64>) -> bool {
  let moved = count.has_changed().expect("the counts are kept while Runners is");
  count.borrow_and_update();
  moved
}

#[test]
fn run_changes_lines_of_one_run_move_its_count_and_the_projects_not_another_runs() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = runners(&runtime);
  let root = Path::new("project");
  let (a, b) = (RunId::generate(), RunId::generate());
  let mut of_a = runners.run_changes_of(root, a);
  let mut of_b = runners.run_changes_of(root, b);
  let mut changes = runners.changes_of(root);
  let mut states = runners.states_of(root);

  runners.count_change_for_test(root, Some(a), false);
  assert!(moved(&mut of_a), "a line of run A moves A's count");
  assert!(moved(&mut changes), "and the project's");
  assert!(!moved(&mut of_b), "not B's");
  assert!(!moved(&mut states), "nor the project's states: no run changed state");

  runners.count_change_for_test(root, Some(b), true);
  assert!(moved(&mut of_b), "a change of run B moves B's count");
  assert!(moved(&mut changes), "and the project's changes");
  assert!(moved(&mut states), "and its states");
  assert!(!moved(&mut of_a), "not A's");
}

#[test]
fn run_changes_a_change_that_names_no_run_moves_every_runs_count() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = runners(&runtime);
  let root = Path::new("project");
  let mut of_a = runners.run_changes_of(root, RunId::generate());
  let mut of_b = runners.run_changes_of(root, RunId::generate());
  let mut changes = runners.changes_of(root);
  let mut states = runners.states_of(root);

  runners.base_notices_queued(root);
  assert!(moved(&mut of_a) && moved(&mut of_b), "queued base notices move every run's count");
  assert!(moved(&mut changes), "and the project's changes");
  assert!(!moved(&mut states), "not its states");

  runners.count_change_for_test(root, None, true);
  assert!(moved(&mut of_a) && moved(&mut of_b), "a run change without a run moves every run's count");
  assert!(moved(&mut changes) && moved(&mut states), "and the project's counts");
}

#[test]
fn run_changes_another_projects_run_moves_nothing_here() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = runners(&runtime);
  let (here, there) = (Path::new("here"), Path::new("there"));
  let run = RunId::generate();
  let mut of_run = runners.run_changes_of(here, run);
  let mut changes = runners.changes_of(here);

  runners.count_change_for_test(there, Some(run), true);
  runners.base_notices_queued(there);
  assert!(!moved(&mut of_run), "another project's changes leave the run's count");
  assert!(!moved(&mut changes), "and the project's");
}

#[test]
fn run_changes_a_run_followed_again_after_its_readers_left_moves_again() {
  let runtime = tokio::runtime::Runtime::new().expect("a runtime");
  let runners = runners(&runtime);
  let root = Path::new("project");
  let run = RunId::generate();
  drop(runners.run_changes_of(root, run));
  runners.count_change_for_test(root, Some(run), false);

  let mut first = runners.run_changes_of(root, run);
  let mut second = first.clone();
  assert!(!moved(&mut first), "a new subscription starts seen");
  runners.count_change_for_test(root, Some(run), false);
  assert!(moved(&mut first) && moved(&mut second), "every receiver of the run's count sees it move");
}
