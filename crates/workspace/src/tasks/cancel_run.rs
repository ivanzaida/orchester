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
  workspace::OpenProject,
};

/// Whether the person confirmed stopping a run for Canceled, and which run the confirmation
/// showed for the task (none when it said the task had no active run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
  Unconfirmed,
  Confirmed(Option<RunId>),
}

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

/// Stops `task`'s active run when it is the run the person confirmed stopping; without an active
/// run there is nothing to stop. A run the confirmation did not show started after it opened.
pub async fn stop_named_run(root: &Path, task: TaskId, stop: Stop, runners: &Runners) -> Result<(), StopRefused> {
  match active_run(root, task, stop).await? {
    None => Ok(()),
    Some(run) => runners
      .control_run(Command::CancelRun { run })
      .await
      .map_err(|error| StopRefused::Failed(crate::tasks::run::control_message(&error))),
  }
}

/// `task`'s active run when it is the one `stop` confirmed; `None` without an active run.
async fn active_run(root: &Path, task: TaskId, stop: Stop) -> Result<Option<RunId>, StopRefused> {
  let root = root.to_path_buf();
  let active = background::run(move || ProjectDb::open(&root).and_then(|db| db.active_task_run(task)))
    .await
    .map_err(StopRefused::Failed)?
    .map_err(|error| StopRefused::Failed(messages::store_error(&error)))?;
  match (active, stop) {
    (None, _) => Ok(None),
    (Some(run), Stop::Unconfirmed) => Err(StopRefused::Unconfirmed(run)),
    (Some(run), Stop::Confirmed(shown)) if shown != Some(run) => Err(StopRefused::StartedSince(run)),
    (Some(run), Stop::Confirmed(_)) => Ok(Some(run)),
  }
}

/// [`stop_named_run`] in `project`, asking its own runner whichever tab shows it.
pub async fn stop_named_run_in(
  project: &OpenProject,
  task: TaskId,
  stop: Stop,
  runners: &Runners,
) -> Result<(), StopRefused> {
  match active_run(&project.repo_root, task, stop).await? {
    None => Ok(()),
    Some(run) => runners
      .control_run_in(project, Command::CancelRun { run })
      .await
      .map_err(|error| StopRefused::Failed(crate::tasks::run::control_message(&error))),
  }
}
