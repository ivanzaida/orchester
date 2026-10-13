use super::{Checkout, LocalError, LocalOperation, LocalPhase};

impl LocalOperation {
  pub const fn task_abort_result(&self) -> Option<&Checkout> {
    self.task_abort_result.as_ref()
  }

  /// Bind abort to the owned merge and its exact clean outcome before Git changes anything.
  pub fn plan_task_abort(&mut self, result: Checkout) -> Result<(), LocalError> {
    if self.phase != LocalPhase::TaskConflicts || self.merged.is_some() || self.pending_resolution.is_some() {
      return Err(LocalError::WrongPhase);
    }
    let before = self.owned_task.as_ref().ok_or(LocalError::Reconcile)?;
    self.ensure_task_owned(before)?;
    if !before
      .operation
      .as_ref()
      .is_some_and(|marker| marker.starts_with("MERGE_HEAD:"))
    {
      return Err(LocalError::ForeignOperation);
    }
    if !result.clean()
      || result.root != before.root
      || result.git_dir != before.git_dir
      || result.branch != before.branch
      || result.head != before.head
      || result.ignored != before.ignored
    {
      return Err(LocalError::Stale);
    }
    self.checked = None;
    self.approved = None;
    self.task_abort_result = Some(result);
    self.needs_reconciliation();
    Ok(())
  }

  pub fn task_abort_finished(&mut self, base: Checkout, task: Checkout) -> Result<(), LocalError> {
    if self.merged.is_some() || self.task_abort_result.as_ref() != Some(&task) {
      return Err(LocalError::Reconcile);
    }
    self.reviewed.ensure_current(&base)?;
    self.review_base(base)?;
    self.owned_task = Some(task);
    self.task_abort_result = None;
    self.task_commit_action = None;
    self.sync_result = None;
    self.paused();
    Ok(())
  }
}
