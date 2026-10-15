//! The in-app local integration application service. Each call takes a project lease and journals intent before Git.

mod abort;
mod authority;
mod cancel;
mod checks;
mod drafts;
mod external;
mod legacy;
mod metadata;
mod poll;
mod preserve;
mod process;
mod recover;
mod resolution;
mod restore;
mod restore_writes;
mod stats;
mod sync;
mod target;
mod task_commit;
mod view;

use std::path::Path;

pub use authority::{LocalAuthorization, LocalIntent, LocalOrigin};
pub use external::{EvidenceInputs, external_integration};
pub use legacy::LegacyPolicy;
pub(crate) use legacy::managed_checkout;
pub use metadata::metadata_allowed;
use orchester_core::{
  id::RunId,
  integration::local::{LocalError, LocalIntegrationStore, LocalOperation, LocalProcessStore},
  run::{Run, RunStatus},
  store::{MergeStore, RunStore},
};
use orchester_exec::local_git::{IntegrationLease, LocalGit, LocalGitError};
use orchester_store::{ProjectDb, StoreError};
pub use poll::fingerprint;
pub use stats::{ShownSnapshot, Snapshot, SnapshotSize, SnapshotStats};
pub use target::recovery_run;
use thiserror::Error;
pub use view::{ConflictView, EditorPolicy, IntegrationView, Preview, pending_restoration, read, read_after};

#[derive(Error)]
pub enum IntegrationError {
  #[error(transparent)]
  State(#[from] LocalError),
  #[error(transparent)]
  Git(#[from] LocalGitError),
  #[error("the local integration journal could not be read or written")]
  Store(#[from] StoreError),
  #[error("sensitive checkout metadata cannot be exposed")]
  SensitiveMetadata,
  #[error("this run is not ready for local integration")]
  Run,
  #[error("External integration evidence requires a reviewed scoped mapping; duplicate integration is unavailable")]
  AlreadyIntegrated,
  #[error("the pinned pipeline has no executable Check tool")]
  NoCheck,
  #[error("fresh Check failed; the result remains unapproved")]
  CheckFailed,
}

impl std::fmt::Debug for IntegrationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

/// Holds a lease only while executing an operation; waiting for a person holds no queue slot.
pub struct LocalIntegration {
  db: ProjectDb,
  run: Run,
  main: LocalGit,
  task: Option<LocalGit>,
  operation: LocalOperation,
  authorization: Option<LocalAuthorization>,
  _lease: IntegrationLease,
  _store_owner: std::sync::Arc<orchester_exec::local_git::owned::StoreOwnership>,
}

impl LocalIntegration {
  pub fn open(root: &Path, run_id: RunId) -> Result<Self, IntegrationError> {
    Self::open_expected(root, run_id, None, None)
  }

  pub fn open_at(root: &Path, run_id: RunId, generation: Option<u64>) -> Result<Self, IntegrationError> {
    Self::open_expected(root, run_id, Some(generation), None)
  }

  pub fn open_authorized_at(
    root: &Path,
    run_id: RunId,
    generation: Option<u64>,
    authorize: LocalAuthorization,
  ) -> Result<Self, IntegrationError> {
    authorize()?;
    Self::open_expected(root, run_id, Some(generation), Some(authorize))
  }

  fn open_expected(
    root: &Path,
    run_id: RunId,
    expected: Option<Option<u64>>,
    authorization: Option<LocalAuthorization>,
  ) -> Result<Self, IntegrationError> {
    let store_owner = orchester_exec::local_git::owned::StoreOwnership::acquire(root)?;
    let lease = IntegrationLease::acquire(root)?;
    let db = ProjectDb::open(root)?;
    let run = db
      .run(run_id)
      .map_err(|_| IntegrationError::Run)?
      .ok_or(IntegrationError::Run)?;
    if run.status() != &RunStatus::Succeeded {
      return Err(IntegrationError::Run);
    }
    ensure_checkout_available(&db, run_id)?;
    let scratch =
      orchester_exec::local_git::prepare_scratch(root, &[".orchester", "local-integration", &run_id.to_string()])?;
    let mut main = LocalGit::new(root, &scratch)?;
    ensure_project_git(&main)?;
    main.own_metadata()?;
    let owned_task =
      orchester_exec::local_git::owned_directory(root, &[".orchester", "worktrees", &run_id.to_string()])?;
    // The project already excludes .orchester in its linked worktrees.
    let task = if let Some(task_root) = owned_task {
      let task_scratch = orchester_exec::local_git::prepare_scratch(&task_root, &[".orchester", "local-integration"])?;
      let mut task = LocalGit::new(&task_root, &task_scratch)?;
      task.own_metadata()?;
      Some(task)
    } else {
      None
    };
    ensure_linked_task(&main, task.as_ref())?;
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
    let saved = db.local_operation(run_id)?;
    if expected.is_some_and(|expected| saved.as_ref().map(|saved| saved.version()) != expected) {
      return Err(LocalError::Stale.into());
    }
    let operation = match saved {
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
    if db.local_operation(run_id)?.is_none()
      && external::contained_result(&db, &main, &operation.reviewed().clone(), &run, None)?.is_some()
    {
      return Err(IntegrationError::AlreadyIntegrated);
    }
    let mut session = Self {
      db,
      run,
      main,
      task,
      operation,
      authorization,
      _lease: lease,
      _store_owner: store_owner,
    };
    session.initialize_journal()?;
    Ok(session)
  }

  fn initialize_journal(&mut self) -> Result<(), IntegrationError> {
    self.ensure_authorized()?;
    if self.db.local_operation(self.run.id())?.is_none() && !self.db.save_local_operation(&self.operation, None)? {
      return Err(LocalError::Stale.into());
    }
    if let Some(merge) = self.db.run_merge(self.run.id())?
      && self.operation.merged().is_none()
    {
      self.operation.reconcile_recorded_merge(&merge)?;
      self.save()?;
    }
    self.bind_process_journal()?;
    Ok(())
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
    self.ensure_authorized()?;
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
fn ensure_checkout_available(db: &ProjectDb, run: RunId) -> Result<(), IntegrationError> {
  for operation in db.local_operations()? {
    if operation.run() != run
      && (operation.reserves_checkout()
        || db
          .local_process(operation.run())?
          .is_some_and(|process| !process.finished))
    {
      return Err(LocalError::ForeignOperation.into());
    }
  }
  Ok(())
}

pub(super) fn ensure_project_git(git: &LocalGit) -> Result<(), IntegrationError> {
  let directory = Path::new(git.common_dir())
    .canonicalize()
    .map_err(|_| LocalGitError::Io)?;
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

fn ensure_linked_task(main: &LocalGit, task: Option<&LocalGit>) -> Result<(), IntegrationError> {
  if let Some(task) = task {
    let common = |git: &LocalGit| -> Result<_, IntegrationError> {
      let name = git.text(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
      Ok(Path::new(&name).canonicalize().map_err(|_| LocalGitError::Io)?)
    };
    if common(main)? != common(task)? {
      return Err(LocalError::Unsupported.into());
    }
  }
  Ok(())
}
