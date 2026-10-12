use orchester_core::{
  integration::local::{Checkout, LocalError, LocalPhase, LocalRecoveryStore, RecoveryLayer},
  store::MergeStore,
};
use orchester_exec::local_git::digest;

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  /// Reconcile facts first. Completed merge identities are never retried.
  pub fn reconcile(&mut self) -> Result<(), IntegrationError> {
    self.operation.resume_saved_phase();
    let actual = self.main.snapshot()?;
    if actual.operation.is_some() {
      return Err(LocalError::ForeignOperation.into());
    }
    if let Some(commit) = self.operation.merged().map(str::to_owned) {
      return self.reconcile_merged(&actual, &commit);
    }
    match self.operation.phase() {
      LocalPhase::Canceling => self.cancel_preservation(),
      LocalPhase::Canceled => self.save(),
      LocalPhase::RemovingPreservation => {
        self.verify_restored()?;
        self.db.remove_local_content(self.run.id())?;
        self.operation.cleanup_finished()?;
        self.save()
      }
      LocalPhase::Merging => self.reconcile_merge(&actual),
      LocalPhase::Committing => self.reconcile_commit(&actual),
      LocalPhase::Preserving => self.reconcile_preservation(&actual),
      LocalPhase::TaskConflicts => {
        if self.operation.pending_resolution().is_some() {
          return self.apply_pending_resolution();
        }
        self.operation.reconcile_task_conflicts(&self.task_git()?.snapshot()?)?;
        self.save()
      }
      LocalPhase::Checking => {
        self.operation.ensure_task_owned(&self.task_git()?.snapshot()?)?;
        self.run_check()
      }
      LocalPhase::Syncing => self.reconcile_sync(),
      _ => {
        self.operation.review_base(actual)?;
        self.save()
      }
    }
  }

  fn reconcile_merged(&mut self, actual: &Checkout, commit: &str) -> Result<(), IntegrationError> {
    if actual.head != commit {
      return Err(LocalError::Stale.into());
    }
    self.record_known_merge(commit)?;
    match self.operation.phase() {
      LocalPhase::RemovingPreservation => {
        self.verify_restored()?;
        self.db.remove_local_content(self.run.id())?;
        self.operation.cleanup_finished()?;
        self.save()
      }
      LocalPhase::Restoring | LocalPhase::RestoreConflicts => {
        self.operation.restoration_conflicts();
        self.save()
      }
      _ => self.save(),
    }
  }

  fn reconcile_merge(&mut self, actual: &Checkout) -> Result<(), IntegrationError> {
    let checked = self.operation.checked().ok_or(LocalError::Reconcile)?.clone();
    if actual.head == checked.result.head && actual.clean() {
      let action = self.operation.merge_action().ok_or(LocalError::Reconcile)?;
      if !self.main.has_merge_receipt(&checked, action)? {
        return Err(LocalError::Reconcile.into());
      }
      self.operation.record_merge(actual.head.clone())?;
      self.save()?;
      return self.record_known_merge(&actual.head);
    }
    if *actual != checked.base {
      return Err(LocalError::Reconcile.into());
    }
    self.operation.review_base(actual.clone())?;
    self.save()
  }

  fn reconcile_commit(&mut self, actual: &Checkout) -> Result<(), IntegrationError> {
    let original = self.operation.reviewed().clone();
    let tree = self.operation.commit_tree().ok_or(LocalError::Reconcile)?.to_owned();
    if *actual == original {
      self.operation.commit_finished(actual.clone())?;
      return self.save();
    }
    if self.main.text(&["rev-parse", "HEAD^{tree}"])? != tree
      || self.main.text(&["rev-parse", "HEAD^"])? != original.head
    {
      return Err(LocalError::Reconcile.into());
    }
    let entries = self.main.index_entries()?;
    let selected = self.operation.commit_selection().to_vec();
    for file in &original.files {
      if self.main.file(&file.path)?.as_deref().map(digest) != file.working
        || (!selected.contains(&file.path) && entries.get(&file.path) != file.staged.as_ref())
      {
        return Err(LocalError::Reconcile.into());
      }
    }
    let committed = self.main.tree_entries(&actual.head)?;
    for path in selected {
      self.main.set_index(&path, committed.get(&path), None)?;
    }
    self.operation.commit_finished(self.main.snapshot()?)?;
    self.save()
  }

  fn reconcile_preservation(&mut self, actual: &Checkout) -> Result<(), IntegrationError> {
    let saved = self.operation.preservation().ok_or(LocalError::NotVerified)?.clone();
    let reviewed = self.operation.reviewed().clone();
    let paths = self.operation.preservation_selection();
    if !saved.verified {
      if actual != &reviewed {
        return Err(LocalError::Stale.into());
      }
      return self.complete_preservation(&reviewed);
    }
    if actual.head != reviewed.head || actual.ignored != reviewed.ignored {
      return Err(LocalError::Reconcile.into());
    }
    let excluded = |checkout: &Checkout| {
      checkout
        .files
        .iter()
        .filter(|file| !paths.contains(&file.path))
        .cloned()
        .collect::<Vec<_>>()
    };
    if excluded(actual) != excluded(&reviewed) {
      return Err(LocalError::Stale.into());
    }
    for file in actual.files.iter().filter(|file| paths.contains(&file.path)) {
      let original = reviewed
        .files
        .iter()
        .find(|original| original.path == file.path)
        .ok_or(LocalError::Stale)?;
      let baseline = self
        .db
        .local_content(self.run.id(), &file.path, RecoveryLayer::WorkingBase)?;
      if (file.working != original.working && file.working != baseline.as_deref().map(digest))
        || (file.staged != original.staged && file.staged != original.head)
      {
        return Err(LocalError::Stale.into());
      }
    }
    let remaining = actual
      .files
      .iter()
      .filter(|file| paths.contains(&file.path))
      .map(|file| file.path.clone())
      .collect::<Vec<_>>();
    let current = if remaining.is_empty() {
      actual.clone()
    } else {
      self.main.clear_preserved_selected(actual, &remaining)?
    };
    self.operation.preservation_finished(current)?;
    self.save()
  }

  fn reconcile_sync(&mut self) -> Result<(), IntegrationError> {
    let task = self.task_git()?.snapshot()?;
    if !task.clean() {
      return Err(LocalError::Reconcile.into());
    }
    let base = self.operation.reviewed().head.clone();
    if self
      .task_git()?
      .text(&["merge-base", "--is-ancestor", &base, &task.head])
      .is_err()
    {
      return Err(LocalError::Reconcile.into());
    }
    self.operation.own_task(task);
    self.operation.begin_check()?;
    self.save()?;
    self.run_check()
  }

  fn record_known_merge(&mut self, commit: &str) -> Result<(), IntegrationError> {
    if self.db.run_merge(self.run.id())?.is_none() {
      self.record_run_merge(commit)?;
    }
    Ok(())
  }

  pub fn pause(&mut self) -> Result<(), IntegrationError> {
    self.operation.paused();
    self.save()
  }
}
