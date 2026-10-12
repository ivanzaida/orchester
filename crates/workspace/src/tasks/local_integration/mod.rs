//! The in-app local integration application service. Each call takes a project lease and journals intent before Git.

mod cancel;
mod checks;
mod preserve;
mod process;
mod recover;
mod resolution;
mod restore;
mod restore_writes;
mod sync;
mod view;

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::local::{LocalError, LocalIntegrationStore, LocalOperation},
  run::{Run, RunStatus},
  store::{MergeStore, RunStore},
};
use orchester_exec::local_git::{IntegrationLease, LocalGit, LocalGitError};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;
pub use view::{ConflictView, EditorPolicy, IntegrationView, Preview, pending_restoration, read};

#[derive(Debug, Error)]
pub enum IntegrationError {
  #[error(transparent)]
  State(#[from] LocalError),
  #[error(transparent)]
  Git(#[from] LocalGitError),
  #[error("the local integration journal could not be read or written")]
  Store(#[from] StoreError),
  #[error("this run is not ready for local integration")]
  Run,
  #[error("the pinned pipeline has no executable Check tool")]
  NoCheck,
  #[error("fresh Check failed; the result remains unapproved")]
  CheckFailed,
}

/// Holds a lease only while executing an operation; waiting for a person holds no queue slot.
pub struct LocalIntegration {
  db: ProjectDb,
  run: Run,
  main: LocalGit,
  task: Option<LocalGit>,
  operation: LocalOperation,
  _lease: IntegrationLease,
}

impl LocalIntegration {
  pub fn open(root: &Path, run_id: RunId) -> Result<Self, IntegrationError> {
    let lease = IntegrationLease::acquire(root)?;
    let db = ProjectDb::open(root)?;
    let run = db
      .run(run_id)
      .map_err(|_| IntegrationError::Run)?
      .ok_or(IntegrationError::Run)?;
    if run.status() != &RunStatus::Succeeded {
      return Err(IntegrationError::Run);
    }
    let scratch =
      orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run_id.to_string()])?;
    let main = LocalGit::new(root, &scratch)?;
    ensure_project_git(&main)?;
    let owned_task =
      orchester_exec::local_git::owned_directory(root, &[".orchester", "worktrees", &run_id.to_string()])?;
    // The project already excludes .orchester in its linked worktrees.
    let task = if let Some(task_root) = owned_task {
      let task_scratch = orchester_exec::local_git::prepare_scratch(&task_root, &[".orchester", "local-integration"])?;
      Some(LocalGit::new(&task_root, &task_scratch)?)
    } else {
      None
    };
    if let Some(task) = &task {
      let common = |git: &LocalGit| -> Result<_, IntegrationError> {
        let name = git.text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
        Ok(Path::new(&name).canonicalize().map_err(|_| LocalGitError::Io)?)
      };
      if common(&main)? != common(task)? {
        return Err(LocalError::Unsupported.into());
      }
    }
    let checkout = main.snapshot()?;
    if run
      .base_branch()
      .is_none_or(|branch| branch.as_str() != checkout.branch)
      || task
        .as_ref()
        .map(|git| git.text(&["symbolic-ref", "--short", "HEAD"]))
        .transpose()?
        .is_some_and(|branch| branch != run.branch().as_str())
    {
      return Err(LocalError::Stale.into());
    }
    let operation = match db.local_operation(run_id)? {
      Some(saved) => {
        if saved.reviewed().root != checkout.root || saved.reviewed().git_dir != checkout.git_dir {
          return Err(LocalError::Stale.into());
        }
        saved
      }
      None => LocalOperation::new(run_id, checkout)?,
    };
    if task.is_none()
      && operation.merged().is_none()
      && operation.preservation().is_none()
      && db.run_merge(run_id)?.is_none()
    {
      return Err(IntegrationError::Run);
    }
    let mut session = Self {
      db,
      run,
      main,
      task,
      operation,
      _lease: lease,
    };
    if session.db.local_operation(run_id)?.is_none() && !session.db.save_local_operation(&session.operation, None)? {
      return Err(LocalError::Stale.into());
    }
    if let Some(merge) = session.db.run_merge(run_id)?
      && session.operation.merged().is_none()
    {
      session.operation.reconcile_recorded_merge(&merge)?;
      session.save()?;
    }
    session.bind_process_journal()?;
    Ok(session)
  }

  pub const fn operation(&self) -> &LocalOperation {
    &self.operation
  }

  fn ensure_latest_run(&self) -> Result<(), IntegrationError> {
    let latest = self
      .db
      .runs_of_task(self.run.task())
      .map_err(|_| IntegrationError::Run)?
      .iter()
      .filter_map(|listed| match listed {
        Ok(run) => Some(run.number().get()),
        Err(unreadable) => unreadable.number.map(|number| number.get()),
      })
      .max();
    if latest != Some(self.run.number().get()) {
      return Err(IntegrationError::Run);
    }
    Ok(())
  }

  fn task_git(&self) -> Result<&LocalGit, IntegrationError> {
    self.task.as_ref().ok_or(IntegrationError::Run)
  }

  fn save(&mut self) -> Result<(), IntegrationError> {
    let version = self.operation.version();
    self.operation.advance_version();
    if !self.db.save_local_operation(&self.operation, Some(version))? {
      return Err(LocalError::Stale.into());
    }
    Ok(())
  }

  pub fn refresh(&mut self) -> Result<(), IntegrationError> {
    self.operation.refresh(self.main.snapshot()?)?;
    self.save()
  }
}
pub(super) fn ensure_project_git(git: &LocalGit) -> Result<(), IntegrationError> {
  let common = git.text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
  let directory = Path::new(&common).canonicalize().map_err(|_| LocalGitError::Io)?;
  for name in ["objects", "refs", "logs"] {
    let path = directory.join(name);
    if path.exists()
      && !path
        .canonicalize()
        .map_err(|_| LocalGitError::Io)?
        .starts_with(git.root())
    {
      return Err(LocalError::Unsupported.into());
    }
  }
  if !directory.starts_with(git.root()) {
    return Err(LocalError::Unsupported.into());
  }
  Ok(())
}
