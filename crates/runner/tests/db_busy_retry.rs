//! A run whose event write meets a busy database (ORC-76): the write fails after it began, as `SQLITE_BUSY` and
//! then `SQLITE_BUSY_SNAPSHOT`, as when another connection took the lock or committed in between. The runner starts
//! the write again, logs each retry, and the run goes on to succeed: it is never interrupted with `runner_failed`.
//! The failures are injected process-wide, so the test has a binary of its own. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test db_busy_retry db_busy_retry`.

mod support;

use std::{fmt, fs, sync::Arc};

use orchester_core::{id::IdempotencyKey, run::RunStatus};
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use rusqlite::ffi;
use tracing::{
  Event, Subscriber,
  field::{Field, Visit},
};
use tracing_subscriber::{
  layer::{Context, Layer, SubscriberExt},
  registry,
};

use self::support::{Setup, project, scheduler, wait_for, wait_for_idle, wait_for_turns};

/// The store's name for the transaction that appends a run's events.
const RUN_EVENTS: &str = "append run events";
/// What the store logs before it starts a busy write again.
const RETRIED: &str = "the database is busy; the write starts again";

#[tokio::test(flavor = "multi_thread")]
async fn db_busy_retry_a_busy_event_write_is_retried_and_the_run_goes_on() {
  let retries = count_retries();
  let project = project(&Setup::new("Wait for release. Append one line."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  orchester_store::fail_next_writes_of_for_test(RUN_EVENTS, &[ffi::SQLITE_BUSY, ffi::SQLITE_BUSY_SNAPSHOT]);
  fs::write(root.join(".orchester").join("fake-release"), "").expect("let the agent finish");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    ProjectDb::open(root).expect("open").runner_failure(id).expect("read"),
    None,
    "the runner did not fail"
  );
  let retried = retries.lock().clone();
  assert!(
    retried.iter().filter(|(what, _)| what == RUN_EVENTS).count() >= 2,
    "both busy errors were retried and logged: {retried:?}"
  );
  assert!(
    retried.iter().any(|(_, code)| code == "SQLITE_BUSY_SNAPSHOT"),
    "the retry names the code: {retried:?}"
  );
}

/// Installs a layer that keeps the transaction name and code of every retry the store logs.
fn count_retries() -> Arc<Mutex<Vec<(String, String)>>> {
  let retries = Arc::new(Mutex::new(Vec::new()));
  let layer = Retries(Arc::clone(&retries));
  tracing::subscriber::set_global_default(registry().with(layer)).expect("the only subscriber of this test");
  retries
}

struct Retries(Arc<Mutex<Vec<(String, String)>>>);

impl<S: Subscriber> Layer<S> for Retries {
  fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
    let mut fields = Fields::default();
    event.record(&mut fields);
    if fields.message == RETRIED {
      self.0.lock().push((fields.what, fields.code));
    }
  }
}

#[derive(Default)]
struct Fields {
  message: String,
  what: String,
  code: String,
}

impl Visit for Fields {
  fn record_str(&mut self, field: &Field, value: &str) {
    match field.name() {
      "what" => value.clone_into(&mut self.what),
      "code" => value.clone_into(&mut self.code),
      _ => {}
    }
  }

  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    if field.name() == "message" {
      self.message = format!("{value:?}");
    }
  }
}
