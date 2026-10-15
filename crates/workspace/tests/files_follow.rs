//! When the code viewer reads a version again: a run's worktree follows the runner's changes while an agent
//! works in it, and only the integration operations once it does not; the checkout and the base branch follow
//! the integration operations; a commit is never read again.

mod support;

use orchester_core::{id::RunId, revision::Revision};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  files::{Follow, Version, agent_at_work},
  tasks::data::ProjectTasks,
};
use support::{Fixture, at};

fn tasks(fixture: &Fixture) -> ProjectTasks {
  ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("the project's tasks")
}

#[test]
fn files_follow_a_runs_worktree_while_its_agent_works_and_the_checkout_on_integration() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Retry webhook deliveries", at(1));
  let mut run = fixture.started_run(task, at(2));
  let working = Version::Run(run.id());
  let read = tasks(&fixture);
  assert!(agent_at_work(&read, run.id()));
  assert_eq!(Follow::of(&working, Some(&read)), Follow::RunChanges);
  assert_eq!(
    Follow::of(&working, None),
    Follow::Integration,
    "before the tasks are read"
  );
  let other = Version::Run(RunId::generate());
  assert_eq!(Follow::of(&other, Some(&read)), Follow::Integration);
  for version in [Version::Checkout, Version::Base] {
    assert_eq!(Follow::of(&version, Some(&read)), Follow::Integration);
  }
  let commit = Version::Revision(Revision::new(&"b".repeat(40)).expect("a commit"));
  assert_eq!(Follow::of(&commit, Some(&read)), Follow::Never);

  fixture.succeed(&mut run, at(3));
  let read = tasks(&fixture);
  assert!(!agent_at_work(&read, run.id()), "the run's agent finished");
  assert_eq!(Follow::of(&working, Some(&read)), Follow::Integration);
}
