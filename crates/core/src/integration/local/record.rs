mod drafts;
mod preservation;
mod privacy;

use serde::{Deserialize, Serialize};

use super::{Checkout, LocalError, RestoredFile};
use crate::id::RunId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalPhase {
  Review,
  Committing,
  Preserving,
  Prepared,
  Syncing,
  TaskConflicts,
  Checking,
  AwaitingApproval,
  Merging,
  MergedAwaitingRestore,
  Restoring,
  RestoreConflicts,
  RestorationVerified,
  RemovingPreservation,
  Complete,
  Paused,
  NeedsReconciliation,
  Canceling,
  Canceled,
}

/// Approval binds to both checkout versions and the committed result tested in the run worktree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedResult {
  pub base: Checkout,
  pub result: Checkout,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preservation {
  pub name: String,
  pub original: Checkout,
  pub verified: bool,
  pub retained: bool,
}

/// Persisted before mutation. Completed merge identity is monotonic and independent of restoration.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalOperation {
  run: RunId,
  version: u64,
  phase: LocalPhase,
  reviewed: Checkout,
  preservation: Option<Preservation>,
  checked: Option<CheckedResult>,
  approved: Option<CheckedResult>,
  merged: Option<String>,
  #[serde(default)]
  merge_action: Option<String>,
  restored: Vec<RestoredFile>,
  sync_attempts: u64,
  owned_task: Option<Checkout>,
  #[serde(default)]
  sync_result: Option<Checkout>,
  #[serde(default)]
  task_commit_action: Option<String>,
  commit_selection: Vec<super::LocalPath>,
  commit_tree: Option<String>,
  paused_from: Option<LocalPhase>,
  #[serde(default)]
  pending_resolution: Option<super::PendingResolution>,
  #[serde(default)]
  preservation_selection: Vec<super::LocalPath>,
  #[serde(default)]
  editor_drafts: Vec<super::DraftRecord>,
}

impl LocalOperation {
  pub fn new(run: RunId, reviewed: Checkout) -> Result<Self, LocalError> {
    if reviewed.operation.is_some() {
      return Err(LocalError::ForeignOperation);
    }
    Ok(Self {
      run,
      version: 0,
      phase: LocalPhase::Review,
      reviewed,
      preservation: None,
      checked: None,
      approved: None,
      merged: None,
      merge_action: None,
      restored: Vec::new(),
      sync_attempts: 0,
      owned_task: None,
      sync_result: None,
      task_commit_action: None,
      commit_selection: Vec::new(),
      commit_tree: None,
      paused_from: None,
      pending_resolution: None,
      preservation_selection: Vec::new(),
      editor_drafts: Vec::new(),
    })
  }

  pub const fn run(&self) -> RunId {
    self.run
  }
  pub const fn version(&self) -> u64 {
    self.version
  }
  pub const fn phase(&self) -> LocalPhase {
    self.phase
  }
  pub const fn reviewed(&self) -> &Checkout {
    &self.reviewed
  }
  pub const fn preservation(&self) -> Option<&Preservation> {
    self.preservation.as_ref()
  }
  pub const fn checked(&self) -> Option<&CheckedResult> {
    self.checked.as_ref()
  }
  pub fn merged(&self) -> Option<&str> {
    self.merged.as_deref()
  }
  pub fn merge_action(&self) -> Option<&str> {
    self.merge_action.as_deref()
  }

  pub fn merge_pending(&self) -> bool {
    self.phase == LocalPhase::Merging
      || (matches!(self.phase, LocalPhase::Paused | LocalPhase::NeedsReconciliation)
        && self.paused_from == Some(LocalPhase::Merging))
  }

  pub fn restored(&self) -> &[RestoredFile] {
    &self.restored
  }
  pub const fn owned_task(&self) -> Option<&Checkout> {
    self.owned_task.as_ref()
  }
  pub fn commit_selection(&self) -> &[super::LocalPath] {
    &self.commit_selection
  }
  pub fn commit_tree(&self) -> Option<&str> {
    self.commit_tree.as_deref()
  }

  /// Read-only reviews release the checkout; mutation and retained recovery keep it reserved.
  pub fn reserves_checkout(&self) -> bool {
    match self.phase {
      LocalPhase::Complete => false,
      LocalPhase::Canceled => self.preservation.as_ref().is_some_and(|saved| saved.retained),
      LocalPhase::Review | LocalPhase::Prepared => {
        self.preservation.is_some()
          || self.merged.is_some()
          || self.commit_tree.is_some()
          || self.owned_task.is_some()
          || self.pending_resolution.is_some()
          || !self.editor_drafts.is_empty()
      }
      _ => true,
    }
  }

  pub const fn sync_result(&self) -> Option<&Checkout> {
    self.sync_result.as_ref()
  }

  /// Save the exact index, operation marker and working bytes expected from the pending sync.
  pub fn plan_sync_result(&mut self, result: Checkout) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Syncing {
      return Err(LocalError::WrongPhase);
    }
    self.sync_result = Some(result);
    Ok(())
  }
  pub fn task_commit_action(&self) -> Option<&str> {
    self.task_commit_action.as_deref()
  }

  pub fn plan_task_commit(&mut self) -> Result<(), LocalError> {
    if self.phase != LocalPhase::TaskConflicts {
      return Err(LocalError::WrongPhase);
    }
    self.task_commit_action = Some(format!("orchester-sync/{}", crate::id::IdempotencyKey::generate()));
    Ok(())
  }
  pub fn commit_intent(&mut self, paths: Vec<super::LocalPath>, tree: String) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Committing {
      return Err(LocalError::WrongPhase);
    }
    self.commit_selection = paths;
    self.commit_tree = Some(tree);
    Ok(())
  }

  pub fn own_task(&mut self, actual: Checkout) {
    self.owned_task = Some(actual);
  }

  pub fn ensure_task_owned(&self, actual: &Checkout) -> Result<(), LocalError> {
    if self.owned_task.as_ref() != Some(actual) {
      return Err(LocalError::ForeignOperation);
    }
    Ok(())
  }

  pub fn paused(&mut self) {
    self.approved = None;
    self.paused_from = Some(self.phase);
    self.phase = LocalPhase::Paused;
  }

  pub fn resume_saved_phase(&mut self) {
    if let Some(phase) = self.paused_from.take() {
      self.phase = phase;
    }
    self.approved = None;
  }

  pub fn review_base(&mut self, current: Checkout) -> Result<(), LocalError> {
    if self.merged.is_some() || !current.clean() {
      return Err(LocalError::WrongPhase);
    }
    self.reviewed = current;
    self.checked = None;
    self.approved = None;
    self.phase = LocalPhase::Prepared;
    Ok(())
  }

  pub fn reconcile_task_conflicts(&mut self, current: &Checkout) -> Result<(), LocalError> {
    self.ensure_task_owned(current)?;
    self.approved = None;
    self.checked = None;
    self.phase = LocalPhase::TaskConflicts;
    Ok(())
  }

  pub fn advance_version(&mut self) {
    self.version += 1;
  }

  pub fn refresh(&mut self, checkout: Checkout) -> Result<(), LocalError> {
    if self.merged.is_some() || self.merge_pending() || self.preservation.as_ref().is_some_and(|saved| saved.retained) {
      return Err(LocalError::WrongPhase);
    }
    if checkout.operation.is_some() {
      return Err(LocalError::ForeignOperation);
    }
    if self.preservation.as_ref().is_some_and(|saved| !saved.retained) {
      self.preservation = None;
      self.restored.clear();
    }
    self.reviewed = checkout;
    self.checked = None;
    self.approved = None;
    self.phase = LocalPhase::Review;
    Ok(())
  }

  pub fn begin_commit(&mut self, current: &Checkout, message: &str) -> Result<(), LocalError> {
    self.prepare_phase(current)?;
    if message.trim().is_empty() {
      return Err(LocalError::Blank);
    }
    self.phase = LocalPhase::Committing;
    Ok(())
  }

  pub fn commit_finished(&mut self, current: Checkout) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Committing {
      return Err(LocalError::WrongPhase);
    }
    self.reviewed = current;
    self.phase = if self.reviewed.clean() {
      LocalPhase::Prepared
    } else {
      LocalPhase::Review
    };
    Ok(())
  }

  pub fn prepared(&mut self, current: Checkout) -> Result<(), LocalError> {
    if !current.clean() {
      return Err(LocalError::Unresolved);
    }
    self.reviewed = current;
    self.phase = LocalPhase::Prepared;
    Ok(())
  }

  fn prepare_phase(&self, current: &Checkout) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Review || self.merged.is_some() {
      return Err(LocalError::WrongPhase);
    }
    self.reviewed.ensure_current(current)
  }

  pub fn begin_sync(&mut self, current: &Checkout) -> Result<(), LocalError> {
    if self.merged.is_some() {
      return Err(LocalError::WrongPhase);
    }
    if !matches!(self.phase, LocalPhase::Prepared | LocalPhase::AwaitingApproval) {
      return Err(LocalError::WrongPhase);
    }
    self.reviewed.ensure_current(current)?;
    if !current.clean() {
      return Err(LocalError::Unresolved);
    }
    self.sync_attempts = self.sync_attempts.checked_add(1).ok_or(LocalError::Reconcile)?;
    self.checked = None;
    self.approved = None;
    self.sync_result = None;
    self.task_commit_action = None;
    self.phase = LocalPhase::Syncing;
    Ok(())
  }

  pub fn task_conflicts(&mut self) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Syncing {
      return Err(LocalError::WrongPhase);
    }
    self.phase = LocalPhase::TaskConflicts;
    Ok(())
  }

  pub fn begin_check(&mut self) -> Result<(), LocalError> {
    if !matches!(
      self.phase,
      LocalPhase::Syncing | LocalPhase::TaskConflicts | LocalPhase::Prepared | LocalPhase::Checking
    ) {
      return Err(LocalError::WrongPhase);
    }
    self.checked = None;
    self.approved = None;
    self.phase = LocalPhase::Checking;
    Ok(())
  }

  pub fn checked_result(&mut self, result: CheckedResult) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Checking || !result.base.clean() || !result.result.clean() {
      return Err(LocalError::NotChecked);
    }
    self.reviewed = result.base.clone();
    self.checked = Some(result);
    self.phase = LocalPhase::AwaitingApproval;
    Ok(())
  }

  pub fn approve(&mut self, exact: &CheckedResult) -> Result<(), LocalError> {
    if self.phase != LocalPhase::AwaitingApproval || self.checked.as_ref() != Some(exact) {
      return Err(LocalError::Stale);
    }
    self.approved = Some(exact.clone());
    Ok(())
  }

  pub fn begin_merge(&mut self, actual: &CheckedResult) -> Result<(), LocalError> {
    if self.merged.is_some() {
      return Err(LocalError::WrongPhase);
    }
    if self.checked.as_ref() != Some(actual) {
      self.approved = None;
      return Err(LocalError::Stale);
    }
    if self.approved.as_ref() != Some(actual) {
      return Err(LocalError::ApprovalRequired);
    }
    self.merge_action = Some(format!("orchester-local/{}", crate::id::IdempotencyKey::generate()));
    self.phase = LocalPhase::Merging;
    Ok(())
  }

  pub fn record_merge(&mut self, commit: String) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Merging || self.checked.as_ref().is_none_or(|exact| exact.result.head != commit) {
      return Err(LocalError::WrongPhase);
    }
    if self.merged.as_ref().is_some_and(|saved| *saved != commit) {
      return Err(LocalError::Reconcile);
    }
    self.merged = Some(commit);
    self.approved = None;
    self.phase = if self.preservation.is_some() {
      LocalPhase::MergedAwaitingRestore
    } else {
      LocalPhase::Complete
    };
    Ok(())
  }

  pub fn reconcile_recorded_merge(&mut self, merge: &super::super::RunMerge) -> Result<(), LocalError> {
    if merge.run != self.run
      || self
        .merged
        .as_ref()
        .is_some_and(|commit| commit != merge.revision.as_str())
    {
      return Err(LocalError::Reconcile);
    }
    self.merged = Some(merge.revision.as_str().to_owned());
    self.approved = None;
    self.phase = if self.preservation.is_some() {
      LocalPhase::MergedAwaitingRestore
    } else {
      LocalPhase::Complete
    };
    Ok(())
  }

  pub fn begin_restore(&mut self) -> Result<(), LocalError> {
    if self.merged.is_none()
      || !matches!(
        self.phase,
        LocalPhase::MergedAwaitingRestore | LocalPhase::RestoreConflicts
      )
    {
      return Err(LocalError::WrongPhase);
    }
    if !self
      .preservation
      .as_ref()
      .is_some_and(|saved| saved.verified && saved.retained)
    {
      return Err(LocalError::NotVerified);
    }
    self.phase = LocalPhase::Restoring;
    Ok(())
  }

  pub fn restored_file(&mut self, file: RestoredFile) -> Result<(), LocalError> {
    if !matches!(
      self.phase,
      LocalPhase::Restoring | LocalPhase::RestoreConflicts | LocalPhase::Canceling
    ) {
      return Err(LocalError::WrongPhase);
    }
    self.restored.retain(|saved| saved.path != file.path);
    self.restored.push(file);
    Ok(())
  }

  pub fn restoration_conflicts(&mut self) {
    self.phase = LocalPhase::RestoreConflicts;
  }

  pub fn restoration_verified(&mut self) -> Result<(), LocalError> {
    if self.merged.is_none() || !matches!(self.phase, LocalPhase::Restoring | LocalPhase::RestoreConflicts) {
      return Err(LocalError::WrongPhase);
    }
    self.phase = LocalPhase::RestorationVerified;
    Ok(())
  }

  pub fn begin_cleanup(&mut self) -> Result<(), LocalError> {
    if !matches!(self.phase, LocalPhase::RestorationVerified | LocalPhase::Canceled) {
      return Err(LocalError::NotVerified);
    }
    self.phase = LocalPhase::RemovingPreservation;
    Ok(())
  }

  pub fn cleanup_finished(&mut self) -> Result<(), LocalError> {
    if self.phase != LocalPhase::RemovingPreservation {
      return Err(LocalError::WrongPhase);
    }
    self.preservation.as_mut().ok_or(LocalError::NotVerified)?.retained = false;
    self.phase = if self.merged.is_some() {
      LocalPhase::Complete
    } else {
      LocalPhase::Canceled
    };
    Ok(())
  }

  pub fn begin_cancel(&mut self, current: &Checkout) -> Result<(), LocalError> {
    if self.merged.is_some()
      || !matches!(
        self.phase,
        LocalPhase::Review | LocalPhase::Prepared | LocalPhase::AwaitingApproval | LocalPhase::Canceling
      )
    {
      return Err(LocalError::WrongPhase);
    }
    let saved = self.preservation.as_ref().ok_or(LocalError::NotVerified)?;
    if !saved.verified || !saved.retained || current.head != saved.original.head {
      return Err(LocalError::Stale);
    }
    self.reviewed.ensure_current(current)?;
    self.checked = None;
    self.approved = None;
    self.phase = LocalPhase::Canceling;
    Ok(())
  }

  pub fn cancellation_verified(&mut self) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Canceling || self.merged.is_some() {
      return Err(LocalError::WrongPhase);
    }
    self.phase = LocalPhase::Canceled;
    Ok(())
  }

  pub const fn pending_resolution(&self) -> Option<&super::PendingResolution> {
    self.pending_resolution.as_ref()
  }

  pub fn plan_resolution(&mut self, plan: super::PendingResolution) -> Result<(), LocalError> {
    if self.phase != LocalPhase::TaskConflicts || self.pending_resolution.is_some() {
      return Err(LocalError::WrongPhase);
    }
    self.ensure_task_owned(&plan.before)?;
    self.pending_resolution = Some(plan);
    Ok(())
  }

  pub fn resolution_finished(&mut self, actual: Checkout) -> Result<(), LocalError> {
    if self.pending_resolution.is_none() {
      return Err(LocalError::WrongPhase);
    }
    self.owned_task = Some(actual);
    self.pending_resolution = None;
    Ok(())
  }

  pub fn needs_reconciliation(&mut self) {
    self.approved = None;
    self.paused_from = Some(self.phase);
    self.phase = LocalPhase::NeedsReconciliation;
  }
}
