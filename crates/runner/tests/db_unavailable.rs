//! A run whose database stays locked longer than a write may wait for it (ORC-74): the write gives up, and the run
//! is interrupted for the runner's failure with the reason in its activity, once the lock is let go. The time a
//! write waits is shortened for the test, process-wide, so the test has a binary of its own. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test db_unavailable db_unavailable`.

mod support;

use std::{fmt, fs, sync::mpsc as blocking, time::Duration};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::IdempotencyKey,
  run::{InterruptCause, RunStatus},
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use tokio::sync::oneshot;
use tracing::{
  Event, Subscriber,
  field::{Field, Visit},
};
use tracing_subscriber::{
  layer::{Context, Layer, SubscriberExt},
  registry,
};

use self::support::{Setup, project, scheduler, wait_for, wait_for_idle, wait_for_turns};

/// How long a write waits for a lock in this test.
const BUDGET: Duration = Duration::from_secs(2);
/// What a run's task logs once the runner failed, before it records the interruption.
const FAILED: &str = "ending workers after runner failure";

#[tokio::test(flavor = "multi_thread")]
async fn db_unavailable_a_lock_held_past_the_budget_interrupts_the_run_and_says_why() {
  let (failed, release) = hold_at_failure();
  orchester_store::set_lock_budget_for_test(BUDGET);
  let project = project(&Setup::new("Wait for release. Append one line."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  let holder = rusqlite::Connection::open(root.join(".orchester").join("orchester.db")).expect("open");
  holder.execute_batch("BEGIN IMMEDIATE").expect("hold the write lock");
  fs::write(root.join(".orchester").join("fake-release"), "").expect("let the agent finish");
  tokio::time::timeout(support::PATIENCE, failed)
    .await
    .expect("a write gives up in time")
    .expect("the layer tells of the failure");
  holder.execute_batch("ROLLBACK").expect("let go of the lock");
  release.send(()).expect("the run's task waits");
  let run = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(run.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
  let details: Vec<String> = ProjectDb::open(root)
    .expect("open")
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::RunnerFailed { detail },
      } => Some(detail),
      _ => None,
    })
    .collect();
  assert!(
    details
      .iter()
      .any(|detail| detail.contains("locked") && detail.contains("SQLITE_BUSY")),
    "the run's activity says the database stayed locked: {details:?}"
  );
}

/// Installs a layer that, the first time a run's task logs [`FAILED`], tells the test and waits until the test
/// let go of the lock: the interruption is recorded after that, not while the lock is still held.
fn hold_at_failure() -> (oneshot::Receiver<()>, blocking::Sender<()>) {
  let (failed, is_failed) = oneshot::channel();
  let (release, released) = blocking::channel();
  let layer = HoldFailed {
    failed: Mutex::new(Some(failed)),
    released: Mutex::new(Some(released)),
  };
  tracing::subscriber::set_global_default(registry().with(layer)).expect("the only subscriber of this test");
  (is_failed, release)
}

struct HoldFailed {
  failed: Mutex<Option<oneshot::Sender<()>>>,
  released: Mutex<Option<blocking::Receiver<()>>>,
}

impl<S: Subscriber> Layer<S> for HoldFailed {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    let mut message = Message::default();
    event.record(&mut message);
    if message.0 != FAILED {
      return;
    }
    let Some(released) = self.released.lock().take() else {
      return;
    };
    if let Some(failed) = self.failed.lock().take() {
      failed.send(()).expect("the test waits for the failure");
    }
    released.recv().expect("the test lets go of the lock");
  }
}

#[derive(Default)]
struct Message(String);

impl Visit for Message {
  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    if field.name() == "message" {
      self.0 = format!("{value:?}");
    }
  }
}
