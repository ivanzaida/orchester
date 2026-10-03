//! Canceling a task stops only the run its confirmation named: a run that started after the
//! confirmation opened is not stopped, and the task is not canceled over it.

use std::path::Path;

use orchester_core::id::{RunId, TaskId};
use orchester_ipc::Command;
use orchester_store::ProjectDb;

use crate::{
  background,
  messages::{self, Message},
  runner::Runners,
};

/// Why Canceled did not go ahead: the task's active run, `run`, was not the one confirmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopRefused {
  /// Nothing was confirmed; the confirmation should name `run`.
  Unconfirmed(RunId),
  /// Another run than the confirmed one is active: it started after the confirmation opened.
  StartedSince(RunId),
  /// The run could not be read or stopped.
  Failed(Message),
}

impl StopRefused {
  /// What the person reads: the confirmation's request, or why the run was not stopped.
  #[must_use]
  pub fn message(&self) -> Message {
    match self {
      Self::Unconfirmed(_) => Message::new(messages::CANCEL_CONFIRM_REQUIRED),
      Self::StartedSince(_) => Message::new("tasks.cancel.startedSince"),
      Self::Failed(message) => message.clone(),
    }
  }

  /// The run the confirmation should name now, when there is one.
  #[must_use]
  pub const fn run(&self) -> Option<RunId> {
    match self {
      Self::Unconfirmed(run) | Self::StartedSince(run) => Some(*run),
      Self::Failed(_) => None,
    }
  }
}

/// Stops `task`'s active run when it is `named`, the run the person confirmed stopping; without
/// an active run there is nothing to stop.
pub async fn stop_named_run(
  root: &Path,
  task: TaskId,
  named: Option<RunId>,
  runners: &Runners,
) -> Result<(), StopRefused> {
  let root = root.to_path_buf();
  let active = background::run(move || ProjectDb::open(&root).and_then(|db| db.active_task_run(task)))
    .await
    .map_err(StopRefused::Failed)?
    .map_err(|error| StopRefused::Failed(messages::store_error(&error)))?;
  match (active, named) {
    (None, _) => Ok(()),
    (Some(run), None) => Err(StopRefused::Unconfirmed(run)),
    (Some(run), Some(named)) if run != named => Err(StopRefused::StartedSince(run)),
    (Some(run), Some(_)) => runners
      .control_run(Command::CancelRun { run })
      .await
      .map_err(|error| StopRefused::Failed(crate::tasks::run::control_message(&error))),
  }
}
