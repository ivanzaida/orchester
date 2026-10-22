//! A shared project's server keeps no *Don't merge automatically* (ORC-69): the task writer refuses the flag there,
//! for a new task, an edit and an organization change, rather than report it saved and have the Orchestrator merge the
//! task after all; the cache keeps the task as it was, an organization change without the flag still goes through, and
//! the project's tasks say the project is shared, so the task form leaves the flag out.

use orchester_core::{
  id::IdempotencyKey,
  store::TaskStore,
  task::organization::{ChangeActor, Priority, TaskOrganization},
  time::Timestamp,
};
use orchester_store::StoreError;
use orchester_workspace::tasks::{
  data::{ProjectTasks, SaveTarget},
  writer::{TaskWriter, WriteError},
};

use super::{
  server_support::TestServer,
  support::{details, shared},
};

fn refused_as_shared<T: std::fmt::Debug>(result: Result<T, WriteError>) {
  match result {
    Err(WriteError::Store(StoreError::SharedProject)) => {}
    other => panic!("refused for the shared project: {other:?}"),
  }
}

#[test]
fn shared_no_automatic_merge_is_refused_and_nothing_is_saved() {
  let server = TestServer::start();
  let alice = shared(&server);
  let _context = alice.runtime.enter();
  let mut writer = TaskWriter::open(alice.root(), &alice.servers).expect("the shared project's writer");
  let task = alice.tasks().remove(0);
  let opted_out = task.organization().clone().with_no_automatic_merge(true);

  refused_as_shared(writer.ensure_merging(task.organization(), &opted_out));
  refused_as_shared(writer.update_organization(task.id(), |value| value.with_no_automatic_merge(true)));
  refused_as_shared(writer.save_organized_by(
    SaveTarget::Task(task.id(), task.revision()),
    details("Keep it for the person"),
    (task.organization(), &opted_out),
    (ChangeActor::Person, Timestamp::now()),
  ));
  let before = alice.tasks().len();
  refused_as_shared(writer.save_organized_by(
    SaveTarget::New(IdempotencyKey::generate()),
    details("A new one kept for the person"),
    (
      &TaskOrganization::default(),
      &TaskOrganization::default().with_no_automatic_merge(true),
    ),
    (ChangeActor::Orchester, Timestamp::now()),
  ));
  assert_eq!(alice.tasks().len(), before, "no task was made");
  assert_eq!(
    alice.db().task(task.id()).expect("read").as_ref(),
    Some(&task),
    "the cache is unchanged"
  );

  writer
    .ensure_merging(
      task.organization(),
      &task.organization().clone().with_priority(Priority::High),
    )
    .expect("another organization change is not refused");
  assert!(
    writer
      .update_organization(task.id(), |value| value.with_priority(Priority::High))
      .expect("saved on the server"),
    "changed"
  );
  let tasks = ProjectTasks::read(&alice.db(), alice.root(), None).expect("tasks");
  assert!(tasks.shared, "the form leaves the flag out");
}
