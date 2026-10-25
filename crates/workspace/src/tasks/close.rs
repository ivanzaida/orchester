//! Closing a task (ORC-91): a person sets it Done or Canceled from its page or the Tasks list, whatever its latest run
//! did (failed, canceled, interrupted or stuck), and reopens it later. A task whose run has not ended asks first
//! whether to stop that run: closing stops only the run the confirmation named ([`super::cancel_run`]), then sets the
//! status. A closed task leaves In progress, Needs you and the queue; its runs and their history stay. A shared task
//! changes on its server, as any change of its organization does.

use std::path::Path;

use orchester_core::{
  id::TaskId,
  store::TaskStore,
  task::{
    TaskStatus,
    organization::{OrganizationError, WorkflowStatus},
  },
};
use orchester_store::StoreError;

use super::{
  cancel_run::{Stop, StopRefused, stop_named_run_in},
  writer::{TaskWriter, WriteError},
};
use crate::{background, runner::Runners, sharing::Servers, workspace::OpenProject};

/// How a person closes a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closing {
  Done,
  Canceled,
}

impl Closing {
  pub const fn status(self) -> WorkflowStatus {
    match self {
      Self::Done => WorkflowStatus::Done,
      Self::Canceled => WorkflowStatus::Canceled,
    }
  }
}

/// Sets `task` of `project` to `closing`'s status, its run stopped first when `stop` confirmed that very run. A task
/// whose run has not ended is refused with that run, for the confirmation to name ([`StopRefused::Unconfirmed`]).
pub async fn close_in(
  project: &OpenProject,
  (runners, servers): (&Runners, &Servers),
  task: TaskId,
  (closing, stop): (Closing, Stop),
) -> Result<(), StopRefused> {
  stop_named_run_in(project, task, stop, runners).await?;
  let (root, servers) = (project.repo_root.clone(), servers.clone());
  let saved = background::run(move || {
    let mut writer = TaskWriter::open(&root, &servers)?;
    writer.update_organization(task, |value| value.with_status(closing.status()))
  })
  .await
  .map_err(StopRefused::Failed)?;
  saved.map(|_| ()).map_err(|error| match error.store() {
    // A run started since the check: the confirmation names it.
    Some(StoreError::Organization(OrganizationError::StopRunFirst(run))) => StopRefused::Unconfirmed(*run),
    _ => StopRefused::Failed(error.message()),
  })
}

/// Reopens `task` of the project at `repo_root`: back to Backlog while it is a draft, to Todo once it was submitted.
/// A task that is not closed stays as it is.
pub fn reopen_in(repo_root: &Path, servers: &Servers, task: TaskId) -> Result<(), WriteError> {
  let mut writer = TaskWriter::open(repo_root, servers)?;
  let found = writer.db().task(task)?.ok_or(StoreError::TaskNotFound(task))?;
  let reopened = match found.status() {
    TaskStatus::Draft => WorkflowStatus::Backlog,
    TaskStatus::Queued => WorkflowStatus::Todo,
  };
  writer.update_organization(task, |value| {
    if value.status().is_closed() {
      value.with_status(reopened)
    } else {
      value
    }
  })?;
  Ok(())
}
