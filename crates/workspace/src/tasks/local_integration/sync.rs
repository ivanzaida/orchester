use orchester_core::{
  integration::{
    MergeKind, RunMerge,
    local::{CheckedResult, LocalError, LocalPhase, Resolution},
  },
  revision::Revision,
  store::MergeStore,
  time::Timestamp,
};

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  /// Syncs base into the owned task worktree; the main checkout does not receive the task yet.
  pub fn sync(&mut self) -> Result<(), IntegrationError> {
    self.ensure_latest_run()?;
    let base = self.main.snapshot()?;
    let task = self.task_git()?.snapshot()?;
    if self.operation.owned_task().is_some() {
      self.operation.ensure_task_owned(&task)?;
    }
    if super::external::contained_result(&self.db, &self.main, &base, &self.run, Some(&self.operation))?.is_some() {
      return Err(IntegrationError::AlreadyIntegrated);
    }
    let main_entries = self.main.tree_entries(&base.head)?;
    let task_entries = self.task_git()?.tree_entries(&task.head)?;
    if main_entries
      .values()
      .chain(task_entries.values())
      .any(|entry| !matches!(entry.mode, 0o100644 | 0o100755))
    {
      return Err(LocalError::Unsupported.into());
    }
    if !task.clean() {
      return Err(LocalError::ForeignOperation.into());
    }
    let result = self.task_git()?.preview_sync(&task, &base.head)?;
    self.operation.begin_sync(&base)?;
    self.operation.own_task(task);
    self.operation.plan_sync_result(result)?;
    self.save()?;
    self.execute_sync()
  }

  pub(super) fn execute_sync(&mut self) -> Result<(), IntegrationError> {
    let base = self.operation.reviewed().head.clone();
    self.operation.ensure_task_owned(&self.task_git()?.snapshot()?)?;
    let merged = self.task_git()?.text(&[
      "-c",
      "merge.conflictStyle=merge",
      "merge",
      "--no-ff",
      "--no-commit",
      "--no-overwrite-ignore",
      &base,
    ]);
    let actual = self.task_git()?.snapshot()?;
    if self.operation.sync_result() != Some(&actual) {
      return Err(LocalError::Reconcile.into());
    }
    if actual.operation.is_some() {
      // Save ownership before committing, including the clean merge whose commit hook can fail.
      self.operation.task_conflicts()?;
      self.operation.own_task(actual);
      self.save()?;
      if !self.task_git()?.conflicts()?.is_empty() {
        return Ok(());
      }
      merged?;
      return self.continue_task();
    }
    merged?;
    self.operation.own_task(actual);
    self.operation.begin_check()?;
    self.save()?;
    self.run_check()
  }

  pub fn save_task_resolution(
    &mut self,
    file: &orchester_core::integration::local::LocalConflict,
    expected: Option<&orchester_core::blob::ContentHash>,
    resolution: &Resolution,
  ) -> Result<(), IntegrationError> {
    if self.operation.phase() != LocalPhase::TaskConflicts {
      return Err(LocalError::WrongPhase.into());
    }
    self.operation.ensure_task_owned(&self.task_git()?.snapshot()?)?;
    // Retain all three original versions before a resolution can remove an unmerged entry.
    for (layer, entry) in [
      (
        orchester_core::integration::local::RecoveryLayer::TaskAncestor,
        &file.ancestor,
      ),
      (
        orchester_core::integration::local::RecoveryLayer::TaskCurrent,
        &file.current,
      ),
      (
        orchester_core::integration::local::RecoveryLayer::TaskIncoming,
        &file.incoming,
      ),
    ] {
      if let Some(entry) = entry {
        let bytes = self.task_git()?.blob(&entry.object)?;
        use orchester_core::integration::local::LocalRecoveryStore;
        self.db.retain_local_content(
          self.run.id(),
          &orchester_core::integration::local::LocalPath::new(&format!(
            "task-originals/{}/{}",
            entry.object, file.path
          ))?,
          layer,
          &orchester_exec::local_git::digest(&bytes),
          &bytes,
        )?;
      }
    }
    self.plan_task_resolution(file, expected, resolution)?;
    self.apply_pending_resolution()
  }

  pub fn continue_task(&mut self) -> Result<(), IntegrationError> {
    self.operation.ensure_task_owned(&self.task_git()?.snapshot()?)?;
    if !self.task_git()?.conflicts()?.is_empty() {
      return Err(LocalError::Unresolved.into());
    }
    self.operation.plan_task_commit()?;
    self.save()?;
    let action = self.operation.task_commit_action().ok_or(LocalError::Reconcile)?;
    self
      .task_git()?
      .commit_task_resolution(super::task_commit::MESSAGE, action)?;
    self.reconcile_task_commit(&self.task_git()?.snapshot()?)
  }

  pub fn approve_and_merge(&mut self, reviewed: &CheckedResult) -> Result<(), IntegrationError> {
    self.approve_and_merge_at(reviewed, self.operation.version())
  }

  pub fn approve_and_merge_at(&mut self, reviewed: &CheckedResult, generation: u64) -> Result<(), IntegrationError> {
    if self.operation.version() != generation {
      return Err(LocalError::Stale.into());
    }
    self.ensure_latest_run()?;
    let actual = CheckedResult {
      base: self.main.snapshot()?,
      result: self.task_git()?.snapshot()?,
    };
    if &actual != reviewed {
      return Err(LocalError::Stale.into());
    }
    self.operation.approve(reviewed)?;
    self.operation.begin_merge(&actual)?;
    self.save()?;
    self.task_git()?.ensure(&actual.result)?;
    self.main.ensure(&actual.base)?;
    let action = self.operation.merge_action().ok_or(LocalError::Reconcile)?.to_owned();
    let merge = self.main.merge_result(&actual, &action);
    let current = self.main.snapshot()?;
    // A failed post-merge hook must not cause a completed merge to be repeated.
    if current.head != actual.result.head {
      merge?;
      return Err(LocalError::Reconcile.into());
    }
    if !self.main.has_merge_receipt(&actual, &action)? {
      return Err(LocalError::Reconcile.into());
    }
    self.operation.record_merge(current.head.clone())?;
    self.save()?;
    self.record_run_merge(&current.head)?;
    merge?;
    Ok(())
  }

  pub(super) fn record_run_merge(&mut self, head: &str) -> Result<(), IntegrationError> {
    if self.db.run_merge(self.run.id())?.is_some() {
      return Ok(());
    }
    self.db.save_run_merge(&RunMerge {
      run: self.run.id(),
      into: self.run.base_branch().ok_or(IntegrationError::Run)?.clone(),
      revision: Revision::new(head).map_err(|_| LocalError::Reconcile)?,
      kind: MergeKind::FastForward,
      merged_at: Timestamp::now(),
    })?;
    Ok(())
  }

  /// Abort is confined to the recorded task worktree version. Additional edits make it unavailable.
  pub fn abort_task(&mut self) -> Result<(), IntegrationError> {
    if self.operation.merged().is_some() || self.operation.phase() != LocalPhase::TaskConflicts {
      return Err(LocalError::WrongPhase.into());
    }
    let actual = self.task_git()?.snapshot()?;
    self.operation.ensure_task_owned(&actual)?;
    if !actual
      .operation
      .as_ref()
      .is_some_and(|marker| marker.starts_with("MERGE_HEAD:"))
    {
      return Err(LocalError::ForeignOperation.into());
    }
    for file in &actual.files {
      if let Some(bytes) = self.task_git()?.file(&file.path)? {
        self.retain_result_bytes(&bytes)?;
      }
    }
    self.operation.ensure_task_owned(&self.task_git()?.snapshot()?)?;
    self.save()?;
    self.task_git()?.text(&["merge", "--abort"])?;
    self.operation.review_base(self.main.snapshot()?)?;
    self.operation.own_task(self.task_git()?.snapshot()?);
    self.operation.paused();
    self.save()
  }
}
