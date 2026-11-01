//! The person's switches over what the Orchestrator chat does itself (`docs/design/orchestrator-toolset.md`,
//! decisions 3 and 5): *Let the Orchestrator publish drafts*, *Let the Orchestrator answer agents*, *Let the
//! Orchestrator merge runs*, *Let the Orchestrator merge finished runs on its own*, *Let the Orchestrator continue
//! interrupted runs* and *Let the Orchestrator write with git* (decision 9). Each is the project's own
//! (`docs/design/settings.md`, ORC-594), kept in its database: a project with none of its own takes the
//! application-wide value it moved from, once, when it opens or is first read. Each is off until a person turns it on
//! in the window, no tool changes it, and it is off in its project from the moment a person turns it off there,
//! whether or not that was saved.
//!
//! Settings › What automation may do shows the application-wide values until its rows move to Project settings
//! (ORC-599, ORC-600): the `app_wide_` methods are that screen's alone, and the Orchestrator never reads them.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_store::{AppDb, OrchestratorSwitch, ProjectDb, StoreError};
use parking_lot::Mutex;

use super::{Consent, Workspace};

/// A switch over what the Orchestrator chat does itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrchestratorConsent {
  /// It publishes the drafts it saved instead of proposing each publication.
  PublishDrafts,
  /// It answers agents' questions and sends a run's agents messages instead of proposing each, and a wait of an
  /// agent starts an Orchestrator turn by itself.
  AnswerAgents,
  /// Its `merge_run` merges at once instead of proposing the merge.
  MergeRuns,
  /// With *merge runs* and *answer agents* also on, the turn a run's success starts merges the run once, when it
  /// passes the chat's rule (`orchestrator::auto_merge`).
  MergeFinished,
  /// Its `resume_run` continues a run a crash, the runner going away or *Quit now* interrupted, from its last
  /// checkpoint; with *answer agents* also on, a turn such an end started continues it once.
  ContinueInterrupted,
  /// Its git tools commit, branch, switch, pull, push, merge, revert, tag and fetch in the project's repository;
  /// what can lose work stays a proposal either way.
  WriteGit,
}

impl OrchestratorConsent {
  pub const ALL: [Self; 6] = [
    Self::PublishDrafts,
    Self::AnswerAgents,
    Self::MergeRuns,
    Self::MergeFinished,
    Self::ContinueInterrupted,
    Self::WriteGit,
  ];

  /// The application-wide value, in `app_settings`.
  fn read(self, db: &AppDb) -> Result<bool, StoreError> {
    match self {
      Self::PublishDrafts => db.orchestrator_publishes_drafts(),
      Self::AnswerAgents => db.orchestrator_answers_agents(),
      Self::MergeRuns => db.orchestrator_merges_runs(),
      Self::MergeFinished => db.orchestrator_merges_finished(),
      Self::ContinueInterrupted => db.orchestrator_continues_interrupted(),
      Self::WriteGit => db.orchestrator_writes_git(),
    }
  }

  fn write(self, db: &mut AppDb, allowed: bool) -> Result<(), StoreError> {
    match self {
      Self::PublishDrafts => db.set_orchestrator_publishes_drafts(allowed),
      Self::AnswerAgents => db.set_orchestrator_answers_agents(allowed),
      Self::MergeRuns => db.set_orchestrator_merges_runs(allowed),
      Self::MergeFinished => db.set_orchestrator_merges_finished(allowed),
      Self::ContinueInterrupted => db.set_orchestrator_continues_interrupted(allowed),
      Self::WriteGit => db.set_orchestrator_writes_git(allowed),
    }
  }

  /// Its column in a project's database.
  const fn switch(self) -> OrchestratorSwitch {
    match self {
      Self::PublishDrafts => OrchestratorSwitch::PublishDrafts,
      Self::AnswerAgents => OrchestratorSwitch::AnswerAgents,
      Self::MergeRuns => OrchestratorSwitch::MergeRuns,
      Self::MergeFinished => OrchestratorSwitch::MergeFinished,
      Self::ContinueInterrupted => OrchestratorSwitch::ContinueInterrupted,
      Self::WriteGit => OrchestratorSwitch::WriteGit,
    }
  }
}

/// A switch's last verified state, by its project's folder (`None`: the application-wide one) and the switch.
type Cells = HashMap<(Option<PathBuf>, OrchestratorConsent), Arc<Mutex<Consent>>>;

/// Each switch's last verified state, by project: a person's revocation holds from their click, before it is saved.
#[derive(Clone, Default)]
pub(super) struct Consents {
  cells: Arc<Mutex<Cells>>,
  /// Held around each save of a project's switch, as the application database's lock is around the application-wide
  /// ones, so the person's choices are stored in the order they were made. A project's switch is read without it:
  /// only what a read returns counts, never what it published, and a card or a turn never waits for another's read.
  saves: Arc<Mutex<()>>,
}

impl Consents {
  fn of(&self, project: Option<&Path>, consent: OrchestratorConsent) -> Arc<Mutex<Consent>> {
    let key = (project.map(Path::to_path_buf), consent);
    Arc::clone(self.cells.lock().entry(key).or_default())
  }
}

impl Workspace {
  /// Whether the person lets the Orchestrator chat do what `consent` names itself in the project at `repo_root`. A
  /// project without a value of its own takes the application-wide one now, and keeps it.
  pub fn orchestrator_allows(&self, repo_root: &Path, consent: OrchestratorConsent) -> Result<bool, StoreError> {
    let result = self.project_switch(repo_root, consent);
    publish_read(&self.orchestrator.of(Some(repo_root), consent), result)
  }

  /// The project's `consent`, read through a connection that cannot write, as cards and turns read it often; only a
  /// project with no value of its own yet, or not migrated yet, is opened to copy the application-wide one in.
  fn project_switch(&self, repo_root: &Path, consent: OrchestratorConsent) -> Result<bool, StoreError> {
    let read = ProjectDb::open_read_only(repo_root).and_then(|db| db.orchestrator_switch(consent.switch()));
    match read {
      Ok(Some(allowed)) => Ok(allowed),
      Ok(None) | Err(StoreError::SchemaTooOld { .. }) => {
        ProjectDb::open(repo_root).and_then(|mut db| self.copy_switch(&mut db, consent))
      }
      Err(error) => Err(error),
    }
  }

  /// Copies into `db` each application-wide switch its project has no value of its own for, as [`Self::open`] does
  /// before anything reads them: a read then only reads, and never writes the project's database while a merge holds
  /// it (`orchester_exec::local_git::owned::StoreOwnership`).
  pub(super) fn copy_orchestrator_switches(&self, db: &mut ProjectDb) -> Result<(), StoreError> {
    OrchestratorConsent::ALL
      .into_iter()
      .try_for_each(|consent| self.copy_switch(db, consent).map(drop))
  }

  /// The project's `consent` in `db`, the application-wide value copied in first where it has none; a copy never
  /// replaces a value the project has.
  fn copy_switch(&self, db: &mut ProjectDb, consent: OrchestratorConsent) -> Result<bool, StoreError> {
    db.copy_orchestrator_switch(consent.switch(), || consent.read(&self.db.lock()))
  }

  /// Saves the person's choice for `consent` in the project at `repo_root`. Only the window's switch, which takes a
  /// person's input alone, calls it.
  pub fn set_orchestrator_allows(
    &self,
    repo_root: &Path,
    consent: OrchestratorConsent,
    allowed: bool,
  ) -> Result<(), StoreError> {
    let cell = self.orchestrator.of(Some(repo_root), consent);
    save(&cell, allowed, || {
      let saves = self.orchestrator.saves.lock();
      let result = ProjectDb::open(repo_root).and_then(|mut db| db.set_orchestrator_switch(consent.switch(), allowed));
      (saves, result)
    })
  }

  /// A person turned `consent` off in the project at `repo_root`: it holds there now, before the save runs or if it
  /// fails.
  pub fn revoke_orchestrator_consent(&self, repo_root: &Path, consent: OrchestratorConsent) {
    self.orchestrator.of(Some(repo_root), consent).lock().revoke();
  }

  /// The application-wide value of `consent`, which Settings › What automation may do shows until ORC-600; projects
  /// copied it once, and the Orchestrator reads its project's own ([`Self::orchestrator_allows`]).
  pub fn app_wide_orchestrator_allows(&self, consent: OrchestratorConsent) -> Result<bool, StoreError> {
    let db = self.db.lock();
    publish_read(&self.orchestrator.of(None, consent), consent.read(&db))
  }

  /// Saves the person's application-wide choice for `consent`; only Settings › What automation may do calls it, and
  /// a project that has its own value keeps it.
  pub fn set_app_wide_orchestrator_allows(
    &self,
    consent: OrchestratorConsent,
    allowed: bool,
  ) -> Result<(), StoreError> {
    let cell = self.orchestrator.of(None, consent);
    save(&cell, allowed, || {
      let mut db = self.db.lock();
      let result = consent.write(&mut db, allowed);
      (db, result)
    })
  }

  /// A person turned the application-wide `consent` off: it holds now, before the save runs or if it fails.
  pub fn revoke_app_wide_orchestrator_consent(&self, consent: OrchestratorConsent) {
    self.orchestrator.of(None, consent).lock().revoke();
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
