//! The person's switch over whether the Orchestrator chat drives (`docs/design/orchestrator-toolset.md`, ORC-909):
//! *Let the Orchestrator drive*. On, the chat acts itself with the tools of the app, and a wait, a run's stop or end
//! and an event start its turn by itself; off, what it would change is a proposal card the person applies and no
//! turn starts by itself. It replaced the six switches (publish drafts, answer agents, merge runs, merge finished runs,
//! continue interrupted runs, write with git). It is the project's own (`docs/design/settings.md`, ORC-594), kept in
//! its database and set in Project settings › Orchestrator: a project with none of its own takes the application-wide
//! *answer agents* value it came from, once, when it opens or is first read; nothing else reads the application-wide
//! value. It is off until a person turns it on in the window, no tool changes it, and it is off in its project from
//! the moment a person turns it off there, whether or not that was saved.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_store::{ProjectDb, StoreError};
use parking_lot::Mutex;

use super::{Consent, Workspace};
use crate::tasks::outside::Outside;

/// Each project's switch's last verified state, by its folder.
type Cells = HashMap<PathBuf, Arc<Mutex<Consent>>>;

/// Each project's switch's last verified state: a person's revocation holds from their click, before it is saved.
#[derive(Clone, Default)]
pub(super) struct Drives {
  cells: Arc<Mutex<Cells>>,
  /// Held around each save of a project's switch, so the person's choices are stored in the order they were made. A
  /// project's switch is read without it: only what a read returns counts, never what it published, and a card or a
  /// turn never waits for another's read.
  saves: Arc<Mutex<()>>,
}

impl Drives {
  fn of(&self, project: &Path) -> Arc<Mutex<Consent>> {
    let mut cells = self.cells.lock();
    let cell = cells
      .entry(project.to_path_buf())
      .or_insert_with(|| Arc::new(Mutex::new(Consent::default())));
    Arc::clone(cell)
  }
}

impl Workspace {
  /// Whether the person lets the Orchestrator chat drive in the project at `repo_root`. A project without a value of
  /// its own takes the application-wide one now, and keeps it.
  pub fn orchestrator_drives(&self, repo_root: &Path) -> Result<bool, StoreError> {
    let result = self.project_drive(repo_root);
    publish_read(&self.orchestrator.of(repo_root), result)
  }

  /// The project's switch, read through a connection that cannot write, as cards and turns read it often; only a
  /// project with no value of its own yet, or not migrated yet, is opened to copy the application-wide one in.
  fn project_drive(&self, repo_root: &Path) -> Result<bool, StoreError> {
    let read = ProjectDb::open_read_only(repo_root).and_then(|db| db.orchestrator_drives());
    match read {
      Ok(Some(drives)) => Ok(drives),
      Ok(None) | Err(StoreError::SchemaTooOld { .. }) => {
        ProjectDb::open(repo_root).and_then(|mut db| self.copy_orchestrator_drives(&mut db))
      }
      Err(error) => Err(error),
    }
  }

  /// The project's switch in `db`, the application-wide value copied in first where it has none, as [`Self::open`]
  /// does before anything reads it: a read then only reads, and never writes the project's database while a merge
  /// holds it (`orchester_exec::local_git::owned::StoreOwnership`); a copy never replaces a value the project has.
  pub(super) fn copy_orchestrator_drives(&self, db: &mut ProjectDb) -> Result<bool, StoreError> {
    db.copy_orchestrator_drives(|| self.db.lock().orchestrator_answers_agents())
  }

  /// Saves the person's choice in the project at `repo_root`, and counts the change once it is published. Only the
  /// window's switch, which takes a person's input alone, calls it.
  pub fn set_orchestrator_drives(&self, repo_root: &Path, drives: bool) -> Result<(), StoreError> {
    let cell = self.orchestrator.of(repo_root);
    let mut project = None;
    let result = save(&cell, drives, || {
      let saves = self.orchestrator.saves.lock();
      let result = ProjectDb::open(repo_root).and_then(|mut db| {
        project = Some(db.project().id());
        db.set_orchestrator_drives(drives)
      });
      (saves, result)
    });
    if let (Ok(()), Some(project)) = (&result, project) {
      self.changes.record(project, Outside::Orchestrator);
    }
    result
  }

  /// A person turned the switch off in the project at `repo_root`: it holds there now, before the save runs or if it
  /// fails.
  pub fn revoke_orchestrator_drive(&self, repo_root: &Path) {
    self.orchestrator.of(repo_root).lock().revoke();
  }
}

/// Publishes in `cell` what a read found and returns what it reports: off while the person's revocation holds.
fn publish_read(cell: &Mutex<Consent>, result: Result<bool, StoreError>) -> Result<bool, StoreError> {
  let allowed = cell.lock().read(matches!(&result, Ok(true)));
  result.map(|_| allowed)
}

/// Saves `allowed` through `write`, which returns its database lock with the result so the outcome is published in
/// `cell` before the lock is let go. Off holds in `cell` from before the save waits for the lock.
fn save<Held>(
  cell: &Mutex<Consent>,
  allowed: bool,
  write: impl FnOnce() -> (Held, Result<(), StoreError>),
) -> Result<(), StoreError> {
  let revision = {
    let mut held = cell.lock();
    if !allowed {
      held.revoke();
    }
    held.begin_save();
    held.revision
  };
  let (_lock, result) = write();
  let mut held = cell.lock();
  if result.is_ok() && held.revision == revision {
    held.revoked = !allowed;
  }
  held.finish_save(allowed && result.is_ok());
  result
}
