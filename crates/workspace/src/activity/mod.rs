//! Background operations of the application, for the status bar (`docs/design/
//! orchester-workspace.md`, "Status bar"): one app-wide [`Activity`] that screens and automation
//! report their work to and the status bar watches.

pub mod bar;
pub mod code_index_operations;
mod log;
pub mod operations;
pub mod skill_operations;

use std::{
  sync::Arc,
  time::{Instant, SystemTime},
};

use tokio::sync::watch;

pub use self::log::{ActivityLog, Event, Operation, OperationId, OperationSpec, Outcome, Place, Slot, Target};
use crate::messages::Message;

/// Shared handle to the session's [`ActivityLog`]; every change is published to subscribers.
#[derive(Clone)]
pub struct Activity {
  log: watch::Sender<ActivityLog>,
}

impl PartialEq for Activity {
  fn eq(&self, other: &Self) -> bool {
    self.log.same_channel(&other.log)
  }
}

impl Default for Activity {
  fn default() -> Self {
    Self {
      log: watch::Sender::new(ActivityLog::default()),
    }
  }
}

impl Activity {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn subscribe(&self) -> watch::Receiver<ActivityLog> {
    self.log.subscribe()
  }

  pub fn snapshot(&self) -> ActivityLog {
    self.log.borrow().clone()
  }

  /// Lists the operation until the returned handle finishes, fails or is dropped.
  pub fn start(&self, spec: OperationSpec) -> Running {
    let mut id = None;
    self.log.send_modify(|log| id = Some(log.start(spec, Instant::now())));
    Running {
      activity: self.clone(),
      id,
    }
  }

  /// Runs `work` as the operation `spec`: its error becomes the failure's reason.
  pub async fn track<T>(
    &self,
    spec: OperationSpec,
    work: impl Future<Output = Result<T, Message>>,
  ) -> Result<T, Message> {
    self.track_to(spec, work, |_| None).await
  }

  /// [`track`](Self::track) for work whose object exists only once it finished: `target`
  /// names it from the result (a created pipeline).
  pub async fn track_to<T>(
    &self,
    spec: OperationSpec,
    work: impl Future<Output = Result<T, Message>>,
    target: impl FnOnce(&T) -> Option<Target>,
  ) -> Result<T, Message> {
    let running = self.start(spec);
    let result = work.await;
    match &result {
      Ok(value) => running.finish_with(target(value)),
      Err(reason) => running.fail(reason.clone()),
    }
    result
  }

  /// Something that finished without being started here, e.g. the runner reconnecting or a run
  /// proposing a plan in `project`.
  pub fn record(&self, project: Option<Arc<str>>, title: Message, target: Target) {
    self
      .log
      .send_modify(|log| log.record(project, title, target, SystemTime::now()));
  }

  /// A run of `project` waits for the user's decision.
  pub fn record_needs_you(&self, project: Option<Arc<str>>, title: Message, target: Target) {
    self
      .log
      .send_modify(|log| log.record_needs_you(project, title, target, SystemTime::now()));
  }

  /// A run at `target` no longer waits for the user's decision: its item goes.
  pub fn settle_needs_you(&self, target: &Target) {
    self.log.send_if_modified(|log| log.settle_needs_you(target));
  }

  /// The user opened the failure's object or dismissed it.
  pub fn resolve(&self, id: OperationId) {
    self.log.send_modify(|log| log.resolve(id));
  }
}

/// An operation in progress. Dropping it without an outcome (the screen that ran it closed)
/// removes it without an event.
pub struct Running {
  activity: Activity,
  id: Option<OperationId>,
}

impl Running {
  pub fn finish(self) {
    self.finish_with(None);
  }

  fn finish_with(mut self, target: Option<Target>) {
    if let Some(id) = self.id.take() {
      self
        .activity
        .log
        .send_modify(|log| log.finish_at(id, target, SystemTime::now()));
    }
  }

  /// Ends the operation as not done, under `title`; see [`ActivityLog::finish_as`].
  pub fn finish_as(mut self, title: Message) {
    if let Some(id) = self.id.take() {
      self
        .activity
        .log
        .send_modify(|log| log.finish_as(id, title, SystemTime::now()));
    }
  }

  pub fn fail(mut self, reason: Message) {
    if let Some(id) = self.id.take() {
      self
        .activity
        .log
        .send_modify(|log| log.fail(id, reason, SystemTime::now()));
    }
  }
}

impl Drop for Running {
  fn drop(&mut self) {
    if let Some(id) = self.id.take() {
      self.activity.log.send_modify(|log| log.cancel(id));
    }
  }
}
