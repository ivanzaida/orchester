//! One start of a project at a time (ORC-885): the person's Run and the recovery of the project's open starts never
//! carry on the same start together; a Run takes the project's turn before its claim; and recovery tries again later
//! after what it could not carry on for now, and leaves a record it cannot read open, settling nothing.

use std::sync::Arc;

use orchester_core::id::{OperationId, RunId};
use orchester_store::StoreError;
use orchester_workspace::{
  runner::RunRequestError,
  team_work::{OpenStarts, StartOutcome, StartTurn, TeamRunner, TeamWork, TeamWorkError},
};
use tokio::sync::Notify;

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{MEMBER, TeamServer},
};

/// Carrying on the open starts did not bring one to its end.
const NOTHING_SETTLED: OpenStarts = OpenStarts { changed: false };

/// A runner that holds the start at its last boundary until the test lets it go on.
#[derive(Clone)]
struct Gated {
  inner: StoreRunner,
  asked: Arc<Notify>,
  go: Arc<Notify>,
}

impl TeamRunner for Gated {
  fn start_accepted_run(&self, operation: OperationId) -> impl Future<Output = Result<RunId, RunRequestError>> + Send {
    let gated = self.clone();
    async move {
      gated.asked.notify_one();
      gated.go.notified().await;
      gated.inner.start_accepted_run(operation).await
    }
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_alone_recovery_leaves_a_start_that_is_under_way_to_its_run() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let gated = Gated {
    inner: runner.clone(),
    asked: Arc::new(Notify::new()),
    go: Arc::new(Notify::new()),
  };
  let (dispatcher, _generation) = checkout.dispatcher(&server, MEMBER);
  let running = TeamWork::new(dispatcher, gated.clone());
  let (dispatcher, _generation) = checkout.dispatcher(&server, MEMBER);
  let recovering = TeamWork::new(dispatcher, gated.clone());

  let request = checkout.start_request(&server, claim);
  let start = checkout.start(&running, request);
  let meanwhile = async {
    gated.asked.notified().await;
    assert_eq!(
      checkout.db().open_team_starts().expect("open").len(),
      1,
      "the start is under way"
    );
    assert!(
      recovering.try_recover(checkout.vault()).await.expect("tried").is_none(),
      "recovery does not take a start Run is working"
    );
    assert_eq!(
      recovering.carry_on_open(checkout.vault()).await.expect("tried"),
      NOTHING_SETTLED
    );
    gated.go.notify_one();
  };
  let (outcome, ()) = tokio::join!(start, meanwhile);

  assert!(matches!(outcome.expect("started"), StartOutcome::Started(_)));
  assert_eq!(runner.asked(), 1, "the runner was asked once");
  assert_eq!(
    recovering.carry_on_open(checkout.vault()).await.expect("tried"),
    NOTHING_SETTLED,
    "nothing is left to carry on once the start ended"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_alone_two_sends_of_one_start_make_one_run() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (first, _generation) = checkout.work(&server, MEMBER, runner.clone());
  let (second, _generation) = checkout.work(&server, MEMBER, runner.clone());
  let request = checkout.start_request(&server, claim);

  let (one, other) = tokio::join!(
    checkout.start(&first, request.clone()),
    checkout.start(&second, request.clone())
  );
  let (StartOutcome::Started(one), StartOutcome::Started(other)) = (one.expect("one"), other.expect("other")) else {
    panic!("both start");
  };
  assert_eq!(one, other, "the same run");
  assert_eq!(runner.asked(), 1, "the runner was asked once, by the first");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_alone_a_start_waiting_for_the_server_is_left_waiting_until_it_answers() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");

  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    NOTHING_SETTLED
  );
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1);

  server.set_down(false);
  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    OpenStarts { changed: true }
  );
  assert!(checkout.db().open_team_starts().expect("open").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_alone_recovery_leaves_the_project_to_a_run_that_holds_its_turn_before_its_claim() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  server.set_down(false);

  // A Run took the turn and is about to claim the task: the recovery of the open start does not run meanwhile.
  let turn = StartTurn::wait(&checkout.servers, checkout.root()).await;
  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    NOTHING_SETTLED
  );
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1, "left alone");
  drop(turn);

  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    OpenStarts { changed: true }
  );
}

#[test]
fn team_work_alone_only_a_record_that_cannot_be_read_is_permanent() {
  assert!(TeamWorkError::Unreadable(OperationId::generate()).is_permanent());
  assert!(!TeamWorkError::NotATask.is_permanent());
  assert!(!TeamWorkError::NotAWork(OperationId::generate()).is_permanent());
  assert!(!TeamWorkError::from(StoreError::TeamStartNotFound(OperationId::generate())).is_permanent());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_alone_a_record_that_cannot_be_read_is_left_open_and_settles_nothing() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db"))
    .expect("open")
    .execute_batch("DROP TRIGGER team_starts_keep_request; UPDATE team_starts SET request = 'not a start'")
    .expect("the record is damaged");
  server.set_down(false);

  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    NOTHING_SETTLED,
    "a record that cannot be read settles nothing"
  );
  assert_eq!(
    checkout.db().open_team_starts().expect("open").len(),
    1,
    "kept, still open"
  );
}
