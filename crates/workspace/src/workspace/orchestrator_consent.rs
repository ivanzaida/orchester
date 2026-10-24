//! The person's switches over what the Orchestrator chat does itself (`docs/design/orchestrator-toolset.md`,
//! decisions 3 and 5), in Settings › What automation may do: *Let the Orchestrator publish drafts*, *Let the
//! Orchestrator answer agents*, *Let the Orchestrator merge runs*, *Let the Orchestrator merge finished runs on its
//! own*, *Let the Orchestrator continue interrupted runs* and *Let the Orchestrator write with git* (decision 9). Each
//! is off until a person turns it on in the window, no tool changes it, and it is off from the moment a person turns it
//! off, whether or not that was saved.

use std::sync::Arc;

use orchester_store::{AppDb, StoreError};
use parking_lot::Mutex;

use super::{Consent, Workspace};

/// A switch over what the Orchestrator chat does itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

/// Each switch's last verified state: a person's revocation holds from their click, before it is saved.
#[derive(Clone, Default)]
pub(super) struct Consents {
  publish: Arc<Mutex<Consent>>,
  answer: Arc<Mutex<Consent>>,
  merge: Arc<Mutex<Consent>>,
  merge_finished: Arc<Mutex<Consent>>,
  continue_interrupted: Arc<Mutex<Consent>>,
  write_git: Arc<Mutex<Consent>>,
}

impl Consents {
  fn of(&self, consent: OrchestratorConsent) -> &Mutex<Consent> {
    match consent {
      OrchestratorConsent::PublishDrafts => &self.publish,
      OrchestratorConsent::AnswerAgents => &self.answer,
      OrchestratorConsent::MergeRuns => &self.merge,
      OrchestratorConsent::MergeFinished => &self.merge_finished,
      OrchestratorConsent::ContinueInterrupted => &self.continue_interrupted,
      OrchestratorConsent::WriteGit => &self.write_git,
    }
  }
}

impl Workspace {
  /// Whether the person lets the Orchestrator chat do what `consent` names itself.
  pub fn orchestrator_allows(&self, consent: OrchestratorConsent) -> Result<bool, StoreError> {
    let db = self.db.lock();
    let result = consent.read(&db);
    let allowed = self.orchestrator.of(consent).lock().read(matches!(&result, Ok(true)));
    result.map(|_| allowed)
  }

  /// Saves the person's choice for `consent`. Only the window's switch, which takes a person's input alone, calls it.
  pub fn set_orchestrator_allows(&self, consent: OrchestratorConsent, allowed: bool) -> Result<(), StoreError> {
    let cell = self.orchestrator.of(consent);
    let revision = {
      let mut held = cell.lock();
      if !allowed {
        held.revoke();
      }
      held.begin_save();
      held.revision
    };
    let mut db = self.db.lock();
    let result = consent.write(&mut db, allowed);
    let mut held = cell.lock();
    if result.is_ok() && held.revision == revision {
      held.revoked = !allowed;
    }
    held.finish_save(allowed && result.is_ok());
    result
  }

  /// A person turned `consent` off: it holds now, before the save runs or if it fails.
  pub fn revoke_orchestrator_consent(&self, consent: OrchestratorConsent) {
    self.orchestrator.of(consent).lock().revoke();
  }
}
