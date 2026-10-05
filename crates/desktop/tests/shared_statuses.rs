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

use orchester_api::{ClaimTake, SharedTask, TaskChanges, TaskPatch};
use orchester_core::{id::TaskId, sharing::ClaimKind, task::organization::WorkflowStatus, time::Timestamp};
use orchester_desktop::sharing::project::WorkClaim;
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

/// Alice takes the run claim of `task`, as starting work on it does.
fn claimed(alice: &Person, task: TaskId) -> WorkClaim {
  let claim = alice
    .block_on(alice.servers.claim_for_work(alice.root(), task))
    .expect("alice holds the task");
  assert!(claim.newly_taken());
  claim
}

/// The review's case: Alice's run reached In review while Oleh canceled the task on the server.
#[test]
fn statuses_a_status_someone_changed_meanwhile_stays_and_the_task_says_so() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  let _held = claimed(&alice, task.id());
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
  let _held = claimed(&alice, task.id());
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

/// Work that did not start leaves no claim behind: the run claim its attempt took is released
/// while no run of the task is under way.
#[test]
fn statuses_a_claim_taken_for_work_that_did_not_start_is_released() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  let claim = claimed(&alice, task.id());
  assert!(on_server(&alice, &oleh, task.id()).claim.is_some());
  alice.block_on(claim.release(&alice.servers));
  assert!(
    on_server(&alice, &oleh, task.id()).claim.is_none(),
    "released on the server"
  );
  let info = alice.db().shared_task_info(task.id()).expect("info").expect("shared");
  assert!(info.claim.is_none(), "and in the cache");
}

/// The review's case: Alice keeps her claim after a finished run (decision 15); Run again confirms
/// it, the runner refuses, and the claim stays hers.
#[test]
fn statuses_a_claim_held_before_work_that_did_not_start_stays() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  let first = claimed(&alice, task.id());
  assert!(first.newly_taken());
  let again = alice
    .block_on(alice.servers.claim_for_work(alice.root(), task.id()))
    .expect("Run again confirms the claim");
  assert!(!again.newly_taken());
  alice.block_on(again.release(&alice.servers));
  assert!(
    on_server(&alice, &oleh, task.id()).claim.is_some(),
    "the claim held before stays"
  );
}

/// A claim taken over a moment ago raises no revision and is not in the cache yet: the tasks are
/// read from the server right before sending, so the former holder sends nothing.
#[test]
fn statuses_a_claim_taken_over_meanwhile_sends_nothing() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  let _held = claimed(&alice, task.id());
  let from = alice.db().task_organization(task.id()).expect("organization").status();
  run_moved(&alice, task.id(), (from, WorkflowStatus::InReview));
  // Alice's other computer lets the task go; Oleh takes it.
  let project = alice.server().link().project;
  let elsewhere = server.sign_in_elsewhere("alice", "alice's own long password", "");
  alice
    .block_on(oleh.client.release_claim(&elsewhere, project, task.id()))
    .expect("released elsewhere");
  alice
    .block_on(oleh.client.claim(
      &oleh.token,
      project,
      task.id(),
      &ClaimTake {
        kind: ClaimKind::Manual,
      },
    ))
    .expect("Oleh takes the task");

  alice.block_on(alice.servers.send_automatic_statuses());
  assert_eq!(
    on_server(&alice, &oleh, task.id()).content.workflow,
    from,
    "nothing was sent"
  );
  assert_eq!(
    alice
      .db()
      .unsent_status(task.id())
      .expect("unsent")
      .map(|unsent| unsent.reason),
    Some(NotSent::NotHolder)
  );
}

/// A claim the person's other computer took a moment ago is not in this computer's cache: the
/// server's answer says it was held already, so a failed start here leaves it alone.
#[test]
fn statuses_a_claim_taken_on_the_other_computer_stays() {
  let server = TestServer::start();
  let alice = shared(&server);
  let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
  member(&alice, "oleh");
  let task = alice.tasks().remove(0);
  let project = alice.server().link().project;
  let elsewhere = server.sign_in_elsewhere("alice", "alice's own long password", "");
  alice
    .block_on(
      oleh
        .client
        .claim(&elsewhere, project, task.id(), &ClaimTake { kind: ClaimKind::Run }),
    )
    .expect("Alice's other computer takes the task");
  // The claim's start and its heartbeat must differ for the server to tell them apart.
  std::thread::sleep(std::time::Duration::from_millis(5));
  let here = alice
    .block_on(alice.servers.claim_for_work(alice.root(), task.id()))
    .expect("confirmed here");
  assert!(!here.newly_taken(), "the other computer took it");
  alice.block_on(here.release(&alice.servers));
  assert!(
    on_server(&alice, &oleh, task.id()).claim.is_some(),
    "the other computer's claim stays"
  );
}
