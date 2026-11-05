//! The executor and its coordinator over a channel that serializes every message, in the runner's own tests
//! (`docs/architecture/remote-runners.md`, rule 4): a scheduler [`Scheduler::over_channel`] hands its runs' tasks a
//! coordinator link, a sink and computer's resources whose every request goes as JSON text to a thread of the
//! coordinator's own, which reads it back, answers it with the local implementation (`LocalLink`, `LocalSink`,
//! `LocalComputer`) and sends the answer back as JSON text, which the executor reads back in turn. So nothing that
//! only works in one process can cross: the executor goes on with what it read back, never with the value it sent or
//! the coordinator made. `crates/runner/tests/serialized_boundary.rs` runs the main run scenarios this way and in
//! process, and compares the rows they leave.
//!
//! What does not cross as text, and why:
//! - The store's errors from the sink (`StoreError`, `RunStoreError`) cross as values: they hold SQLite's own errors
//!   and do not serialize. A remote sink needs an error of its own that does; the scenarios meet none.
//! - Another link or sink ([`CoordinatorLink::another`], [`Sink::another`]) is a new connection the coordinator's side
//!   opens, not a message: it gets a thread of its own like the first.
//! - The computer's secrets, their values and its MCP servers (`ComputerResources::secret_value`, `secret_screen`,
//!   `secret`, `secrets`, `mcp_server`, `mcp_servers`) stay on the executor's side: a value is never sent, and `Secret`
//!   and `McpServer` do not serialize.
//!
//! [`CoordinatorLink::another`]: crate::execution::executor::link::CoordinatorLink::another
//! [`Sink::another`]: crate::execution::executor::sink::Sink::another

mod computer;
mod link;
mod sink;

use std::{
  any::Any,
  collections::BTreeMap,
  sync::{Arc, mpsc},
  thread,
};

use parking_lot::Mutex;
use serde::{Serialize, de::DeserializeOwned};

pub(in crate::execution) use self::{link::ChannelLink, sink::ChannelSink};
use super::super::Scheduler;
use crate::lock_pause::off_async_workers;

/// How many requests of each kind crossed the channel of one scheduler.
#[derive(Default)]
pub(in crate::execution) struct Calls(Mutex<BTreeMap<&'static str, usize>>);

impl Calls {
  fn count(&self, method: &'static str) {
    *self.0.lock().entry(method).or_default() += 1;
  }
}

impl Scheduler {
  /// This scheduler with its runs' executor reaching the coordinator over a channel that serializes every message,
  /// before it is shared: what the runner's tests of the boundary give it.
  #[must_use]
  pub fn over_channel(mut self) -> Self {
    if let Some(inner) = Arc::get_mut(&mut self.inner) {
      let calls = Arc::new(Calls::default());
      inner.computer = Arc::new(computer::ChannelComputer::over(inner.computer.clone(), calls.clone()));
      inner.channel = Some(calls);
    }
    self
  }

  /// How many requests of each kind crossed this scheduler's channel ([`Scheduler::over_channel`]), by the name of
  /// the link's, the sink's or the computer's method; empty for a scheduler without one.
  pub fn sent_over_channel(&self) -> BTreeMap<String, usize> {
    let Some(calls) = &self.inner.channel else {
      return BTreeMap::new();
    };
    calls
      .0
      .lock()
      .iter()
      .map(|(method, count)| ((*method).to_owned(), *count))
      .collect()
  }
}

/// What the coordinator's side answers: the answer as JSON text, or an error that crosses as a value.
type Reply = Result<String, Box<dyn Any + Send>>;

/// What a request asks of the coordinator's side, which holds `B`.
type Work<B> = Box<dyn FnOnce(&mut B) + Send>;

/// The executor's end of a channel to a thread of the coordinator's that holds `B` and answers each request with
/// `serve`. The thread ends once this end is dropped.
struct Wire<B> {
  calls: mpsc::Sender<Work<B>>,
  serve: fn(&mut B, &str, &str) -> Reply,
  counted: Arc<Calls>,
}

impl<B: Send + 'static> Wire<B> {
  fn open(backend: B, serve: fn(&mut B, &str, &str) -> Reply, counted: Arc<Calls>) -> Self {
    let (calls, received) = mpsc::channel::<Work<B>>();
    thread::Builder::new()
      .name("coordinator side of the channel".to_owned())
      .spawn(move || {
        let mut backend = backend;
        for work in received {
          work(&mut backend);
        }
      })
      .expect("the coordinator's side of the channel starts");
    Self { calls, serve, counted }
  }

  /// Sends `request` to the coordinator's side as JSON and reads its answer back from JSON; an error that crosses as
  /// a value comes back as it is.
  fn ask<R: Serialize, A: DeserializeOwned>(
    &self,
    method: &'static str,
    request: &R,
  ) -> Result<A, Box<dyn Any + Send>> {
    self.counted.count(method);
    let text = serde_json::to_string(request).expect("a request over the channel serializes");
    let serve = self.serve;
    let answer = self.on_coordinator_side(move |backend| serve(backend, method, &text))?;
    Ok(serde_json::from_str(&answer).expect("an answer over the channel deserializes"))
  }

  /// What `work` gives on the coordinator's side, waited for without holding up the runtime's other tasks.
  fn on_coordinator_side<T: Send + 'static>(&self, work: impl FnOnce(&mut B) -> T + Send + 'static) -> T {
    let (reply, replied) = mpsc::channel();
    let work: Work<B> = Box::new(move |backend| {
      // The executor's side waits for the reply while its end is open; it is gone only if it panicked.
      if reply.send(work(backend)).is_err() {
        tracing::warn!("the executor's side of the channel left before its answer");
      }
    });
    self
      .calls
      .send(work)
      .expect("the coordinator's side serves while the executor's end is open");
    off_async_workers(|| replied.recv()).expect("the coordinator's side answers every request")
  }
}

/// The request in `text`, as the coordinator's side reads it.
fn received<R: DeserializeOwned>(text: &str) -> R {
  serde_json::from_str(text).expect("a request over the channel deserializes")
}

/// `answer`, an error included, as JSON text: what the link and the computer answer, whose errors serialize.
fn whole<A: Serialize>(answer: &A) -> Reply {
  Ok(serde_json::to_string(answer).expect("an answer over the channel serializes"))
}

/// `answer` as JSON text, or its error as a value: what the sink answers, whose errors do not serialize.
fn written<A: Serialize, E: Send + 'static>(answer: Result<A, E>) -> Reply {
  match answer {
    Ok(answer) => whole(&answer),
    Err(error) => Err(Box::new(error)),
  }
}

/// An answer that crossed whole ([`whole`]).
fn crossed_whole<A>(answer: Result<A, Box<dyn Any + Send>>) -> A {
  answer.unwrap_or_else(|_| panic!("the link and the computer answer as JSON, their errors too"))
}

/// The error `E` the sink's method gave, back from its value ([`written`]).
fn kept_error<E: 'static>(error: Box<dyn Any + Send>) -> E {
  *error
    .downcast::<E>()
    .unwrap_or_else(|_| panic!("the sink's method gives its own error"))
}
