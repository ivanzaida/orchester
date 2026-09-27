//! A request to continue a run that arrives once the run is recorded as stopped, but before the
//! task that stopped it has ended: the desktop reads the stop as soon as it is announced, while
//! the task still finishes, which on a busy machine can take a while. The request waits for the
//! task instead of being refused as if the run were still executing. The task is held at its
//! last step through the runner's log, so the moment does not depend on timing. Needs `git`. Run
//! with `cargo test -p orchester-runner --features test-agent --test settling`.

mod support;

use std::{fmt, future::Future, sync::mpsc as blocking, task::Poll, time::Duration};

use orchester_core::{
  id::IdempotencyKey,
  run::{PauseReason, RaisedLimits, RunLimit, RunStatus},
  store::RunStore,
  tokens::TokenBudget,
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

use self::support::{PATIENCE, Setup, project, scheduler, wait_for};

/// What a run's task logs as its last step, once it has recorded and announced how the run
/// settled.
const SETTLED: &str = "run settled";

/// Holds the first task that logs [`SETTLED`] there until the test lets it go.
struct HoldSettled {
  held: Mutex<Option<oneshot::Sender<()>>>,
  release: Mutex<Option<blocking::Receiver<()>>>,
}

impl<S: Subscriber> Layer<S> for HoldSettled {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    let mut message = Message::default();
    event.record(&mut message);
    if message.0 != SETTLED {
      return;
    }
    let Some(release) = self.release.lock().take() else {
      return;
    };
    if let Some(held) = self.held.lock().take() {
      held.send(()).expect("the test waits for the task to be held");
    }
    release.recv().expect("the test lets the task go");
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

#[tokio::test(flavor = "multi_thread")]
async fn settling_a_raise_waits_for_the_task_that_stopped_the_run() {
  let (held, is_held) = oneshot::channel();
  let (release, released) = blocking::channel();
  let hold = HoldSettled {
    held: Mutex::new(Some(held)),
    release: Mutex::new(Some(released)),
  };
  tracing::subscriber::set_global_default(registry().with(hold)).expect("the only subscriber of this test");
  let project = project(&Setup {
    budget: 5_000,
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  tokio::time::timeout(PATIENCE, is_held)
    .await
    .expect("the run stops at its token limit in time")
    .expect("the task is held");
  let stopped = ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run");
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
  );

  let limits = RaisedLimits {
    budget: TokenBudget::new(2_000_000).expect("above zero"),
    max_duration: Duration::from_secs(30 * 60),
  };
  let raise = scheduler.raise_limit(id, limits);
  tokio::pin!(raise);
  let first = std::future::poll_fn(|cx| Poll::Ready(raise.as_mut().poll(cx))).await;
  assert!(first.is_pending(), "the raise waits for the task: {first:?}");
  release.send(()).expect("the task is held");
  assert_eq!(raise.await, Ok(()));
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
}
