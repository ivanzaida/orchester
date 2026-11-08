//! One start of a project at a time (ORC-885): the person's Run and the recovery of the project's open starts never
//! carry on the same start together, and recovery tries again later after what it could not carry on.

use std::sync::Arc;

use orchester_core::id::{OperationId, RunId};
use orchester_workspace::{
  runner::RunRequestError,
  team_work::{OpenStarts, StartOutcome, TeamRunner, TeamWork},
};
use tokio::sync::Notify;

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{MEMBER, TeamServer},
};

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
  let start = running.start(request, checkout.vault());
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
      OpenStarts::Waiting
    );
    gated.go.notify_one();
  };
  let (outcome, ()) = tokio::join!(start, meanwhile);

  assert!(matches!(outcome.expect("started"), StartOutcome::Started(_)));
  assert_eq!(runner.asked(), 1, "the runner was asked once");
  assert_eq!(
    recovering.carry_on_open(checkout.vault()).await.expect("tried"),
    OpenStarts::Settled,
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
    first.start(request.clone(), checkout.vault()),
    second.start(request.clone(), checkout.vault())
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
  work
    .start(checkout.start_request(&server, claim), checkout.vault())
    .await
    .expect("waits");

  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    OpenStarts::Waiting
  );
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1);

  server.set_down(false);
  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    OpenStarts::Settled
  );
  assert!(checkout.db().open_team_starts().expect("open").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_alone_a_start_that_could_not_be_carried_on_is_tried_again() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  work
    .start(checkout.start_request(&server, claim), checkout.vault())
    .await
    .expect("waits");
  rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db"))
    .expect("open")
    .execute_batch("DROP TRIGGER team_starts_keep_request; UPDATE team_starts SET request = 'not a start'")
    .expect("the record is damaged");
  server.set_down(false);

  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    OpenStarts::Waiting,
    "an error of one start does not end the retries"
  );
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1, "still open");
}
