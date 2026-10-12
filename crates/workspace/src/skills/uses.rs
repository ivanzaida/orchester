//! Skill versions retained by unfinished runs, rather than the latest version now.
use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  resource::VersionNumber,
  run::holds_task,
  store::{RunStore, TaskStore, WorktreeStore},
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};
#[derive(Debug, thiserror::Error)]
pub enum UseError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Run(#[from] RunStoreError),
}
impl UseError {
  pub fn message(&self) -> crate::messages::Message {
    match self {
      Self::Store(e) => crate::messages::store_error(e),
      Self::Run(e) => {
        tracing::error!(%e,"cannot read active skill use");
        crate::messages::Message::new("errors.storage")
      }
    }
  }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveUse {
  pub skill: ResourceId,
  pub task: Arc<str>,
  pub version: VersionNumber,
  pub step: Option<Arc<str>>,
}
pub fn active(db: &ProjectDb) -> Result<Vec<ActiveUse>, UseError> {
  let mut uses = Vec::new();
  for task in db.tasks()? {
    for run in db.runs_of_task(task.id())? {
      let run = run.map_err(|unreadable| unreadable.error)?;
      if !holds_task(&run, db.worktree_removal(run.id())?.is_some()) {
        continue;
      }
      for pinned in run.snapshot().skills() {
        let step = run
          .current_attempt()
          .and_then(|attempt| attempt.executions().last())
          .and_then(|e| run.snapshot().body().steps().iter().find(|s| &s.key == e.step()))
          .map(|s| s.name.as_str().into());
        uses.push(ActiveUse {
          skill: pinned.resource,
          task: task.key().to_string().into(),
          version: pinned.number,
          step,
        });
      }
    }
  }
  Ok(uses)
}
