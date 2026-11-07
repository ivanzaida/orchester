//! The check every start of a task in a window makes before it queues, claims or hands the task to a runner: a
//! pipeline that is not ready on this computer starts nothing and counts no attempt (`not_ready_here`), as the team
//! start and `run_task` refuse it. The read blocks (it stats program files and asks the keychain), so it runs on the
//! background pool, once for each start.

use std::path::Path;

use orchester_core::{
  id::{ResourceId, TaskId},
  secret::SecretVault,
};

use super::{NotReadyHere, ReadinessError};
use crate::{background, messages::Message, secrets::Vault, workspace::Workspace};

/// Why a start was refused before anything was queued, claimed or started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotStarted {
  /// The pipeline is not ready here: the parts that are missing, each with its fix.
  NotReadyHere(NotReadyHere),
  /// Whether the pipeline is ready could not be read, so nothing is started on a guess.
  Unreadable(Message),
}

impl NotStarted {
  /// Why nothing started, then each part with its fix.
  pub fn messages(&self) -> Vec<Message> {
    match self {
      Self::NotReadyHere(refusal) => refusal.messages(),
      Self::Unreadable(message) => vec![message.clone()],
    }
  }

  /// What one line can say of it, where there is room for no list of parts.
  pub fn short(&self) -> Message {
    match self {
      Self::NotReadyHere(refusal) => refusal.short(),
      Self::Unreadable(message) => message.clone(),
    }
  }
}

impl Workspace {
  /// `Ok` when the pipeline of `task` of the project at `repo_root` may start here now; a task with no pipeline, or a
  /// pipeline with no version, is left to the preflight, which refuses it with its own reason.
  pub async fn ready_to_start(&self, repo_root: &Path, vault: &Vault, task: TaskId) -> Result<(), NotStarted> {
    self
      .refuse_unless_ready(repo_root, vault, move |workspace, root, vault| {
        workspace.task_not_ready_here(root, vault, task)
      })
      .await
  }

  /// [`Self::ready_to_start`] for a task not stored yet (Create & run), by the `pipeline` it names.
  pub async fn ready_to_start_pipeline(
    &self,
    repo_root: &Path,
    vault: &Vault,
    pipeline: Option<ResourceId>,
  ) -> Result<(), NotStarted> {
    self
      .refuse_unless_ready(repo_root, vault, move |workspace, root, vault| match pipeline {
        Some(pipeline) => workspace.not_ready_here(root, vault, pipeline),
        None => Ok(None),
      })
      .await
  }

  async fn refuse_unless_ready(
    &self,
    repo_root: &Path,
    vault: &Vault,
    read: impl FnOnce(&Workspace, &Path, &dyn SecretVault) -> Result<Option<NotReadyHere>, ReadinessError> + Send + 'static,
  ) -> Result<(), NotStarted> {
    let (workspace, root, vault) = (self.clone(), repo_root.to_owned(), vault.clone());
    let read = background::run(move || read(&workspace, &root, vault.get()))
      .await
      .map_err(NotStarted::Unreadable)?;
    match read {
      Ok(None) => Ok(()),
      Ok(Some(refusal)) => Err(NotStarted::NotReadyHere(refusal)),
      Err(error) => Err(NotStarted::Unreadable(error.message())),
    }
  }
}
