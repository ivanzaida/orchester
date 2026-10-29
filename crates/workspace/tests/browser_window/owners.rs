//! Who owns a run's tabs, as the Browser window names them (`orchester_workspace::browser::run_owners`): the run's task
//! key and number, the agent profile of the step at work, and whether the run waits for the person, read from the
//! project's database.

use orchester_core::{id::RunId, store::TaskStore};
use orchester_workspace::browser::run_owners;

use crate::support::{Fixture, at};

#[test]
fn browser_window_owners_name_the_run_its_agent_and_whether_it_waits() {
  let mut fixture = Fixture::gated();
  let task = fixture.task("Sync orders", at(10));
  let mut run = fixture.started_run(task, at(20));
  let key = fixture
    .db
    .task(task)
    .expect("read")
    .expect("the task")
    .key()
    .to_string();
  let gone = RunId::generate();

  let owners = run_owners(fixture.repo.path(), &[run.id(), gone]).expect("read");
  assert_eq!(owners.len(), 1, "a run the database does not have is left out");
  let owner = &owners[&run.id()];
  assert_eq!(owner.key.as_ref(), key);
  assert_eq!(owner.number, run.number());
  assert_eq!(
    owner.agent.as_deref(),
    Some("Builder"),
    "the profile of the step at work"
  );
  assert!(!owner.waits, "the agent works");

  fixture.propose(&mut run, "1. Page the orders.", at(30));
  let owners = run_owners(fixture.repo.path(), &[run.id()]).expect("read");
  assert!(
    owners[&run.id()].waits,
    "a step waiting for a plan decision waits for the person"
  );
}
