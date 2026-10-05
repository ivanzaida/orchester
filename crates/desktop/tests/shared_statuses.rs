//! The workflow statuses this computer's runs moved shared tasks to (`docs/architecture/server.md`,
//! "Claims"), against a real `orchester-server` on loopback: one is sent as Orchester's change only
//! while the server still has the status the run changed from and the person holds the task; a
//! status someone else set meanwhile stays, and the task says why the run's was not sent.
//!
//! Work that did not start releases the run claim taken for it.
//!
//! `cargo test -p orchester-desktop --test shared_statuses statuses_`

#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use orchester_api::{SharedTask, TaskChanges, TaskPatch};
use orchester_core::{id::TaskId, task::organization::WorkflowStatus, time::Timestamp};
use orchester_store::{AutomaticStatus, NotSent};
use server_support::TestServer;
use support::{Other, Person, member, shared};

/// Leaves the row a run of `task` writes when it moves the shared task from `from` to `status`, as
/// the runner's store write does (`ProjectDb::append_events`); the desktop has no run here.
fn run_moved(alice: &Person, task: TaskId, (from, status): (WorkflowStatus, WorkflowStatus)) -> Timestamp {
  let at = Timestamp::now();
  let encode = |status: WorkflowStatus| serde_json::json!({ "schema": 1, "body": status }).to_string();
  let connection = rusqlite::Connection::open(alice.root().join(".orchester").join("orchester.db")).expect("database");
  connection
    .execute(
      "INSERT INTO shared_automatic_status (task_id, from_status, status, changed_at) VALUES (?1, ?2, ?3, ?4)",
      rusqlite::params![task.to_string(), encode(from), encode(status), at.unix_millis()],
    )
    .expect("the run's status");
  at
}

fn on_server(alice: &Person, oleh: &Other, task: TaskId) -> SharedTask {
  let project = alice.server().link().project;
  alice
    .block_on(oleh.client.tasks(&oleh.token, project))
    .expect("tasks")
    .tasks
    .into_iter()
    .find(|shared| shared.id == task)
    .expect("the task")
}

fn claimed(alice: &Person, task: TaskId) {
  alice
    .block_on(alice.servers.claim_for_work(alice.root(), task))
    .expect("alice holds the task");
}

/// The review's case: Alice's run reached In review while Oleh canceled the task on the server.
#[test]
fn statuses_a_status_someone_changed_meanwhile_stays_and_the_task_says_so() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  claimed(&alice, task.id());
  let from = alice.db().task_organization(task.id()).expect("organization").status();
  run_moved(&alice, task.id(), (from, WorkflowStatus::InReview));
  let current = on_server(&alice, &oleh, task.id());
  alice
    .block_on(oleh.client.update_task(
      &oleh.token,
      alice.server().link().project,
      task.id(),
      &TaskPatch {
        revision: current.revision,
        changes: TaskChanges {
          workflow: Some(WorkflowStatus::Canceled),
          ..TaskChanges::default()
        },
      },
    ))
    .expect("Oleh cancels the task");

  alice.block_on(alice.server().sync()).expect("sync");
  assert_eq!(
    on_server(&alice, &oleh, task.id()).content.workflow,
    WorkflowStatus::Canceled,
    "Oleh's change stays"
  );
  assert!(alice.db().automatic_statuses().expect("pending").is_empty());
  let unsent = alice.db().unsent_status(task.id()).expect("unsent").expect("shown");
  assert_eq!(
    unsent.reason,
    NotSent::Changed {
      current: WorkflowStatus::Canceled
    }
  );
  assert_eq!(unsent.status, WorkflowStatus::InReview);
}

#[test]
fn statuses_a_held_task_s_status_goes_as_soon_as_it_is_asked_for() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(1);
  claimed(&alice, task.id());
  let from = alice.db().task_organization(task.id()).expect("organization").status();
  run_moved(&alice, task.id(), (from, WorkflowStatus::InReview));
  alice.block_on(alice.servers.send_automatic_statuses());
  let sent = on_server(&alice, &oleh, task.id());
  assert_eq!(sent.content.workflow, WorkflowStatus::InReview);
  assert!(alice.db().automatic_statuses().expect("pending").is_empty());
  assert_eq!(alice.db().unsent_status(task.id()).expect("unsent"), None);
}

#[test]
fn statuses_a_task_the_person_no_longer_holds_is_left_alone() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(2);
  let from = alice.db().task_organization(task.id()).expect("organization").status();
  let at = run_moved(&alice, task.id(), (from, WorkflowStatus::InReview));
  alice.block_on(alice.server().sync()).expect("sync");
  assert_eq!(on_server(&alice, &oleh, task.id()).content.workflow, from);
  assert_eq!(
    alice
      .db()
      .unsent_status(task.id())
      .expect("unsent")
      .map(|unsent| (unsent.reason, unsent.at)),
    Some((NotSent::NotHolder, at))
  );
  assert_eq!(
    alice.db().automatic_statuses().expect("pending"),
    Vec::<AutomaticStatus>::new()
  );
}

/// Work that did not start leaves no claim behind: the run claim taken for it is released while no
/// run of the task is under way.
#[test]
fn statuses_a_claim_for_work_that_did_not_start_is_released() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  claimed(&alice, task.id());
  assert!(on_server(&alice, &oleh, task.id()).claim.is_some());
  alice.block_on(alice.servers.release_unused_claim(alice.root(), task.id()));
  assert!(
    on_server(&alice, &oleh, task.id()).claim.is_none(),
    "released on the server"
  );
  let info = alice.db().shared_task_info(task.id()).expect("info").expect("shared");
  assert!(info.claim.is_none(), "and in the cache");
}
