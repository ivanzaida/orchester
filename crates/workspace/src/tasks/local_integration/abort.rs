use orchester_core::integration::local::{LocalError, LocalPhase};

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  /// Abort only the recorded task merge; keep named preservation and edited results retained.
  pub fn abort_task(&mut self) -> Result<(), IntegrationError> {
    if self.operation.task_abort_result().is_some() {
      self.operation.resume_saved_phase();
      return self.reconcile_task_abort();
    }
    if self.operation.merged().is_some() || self.operation.phase() != LocalPhase::TaskConflicts {
      return Err(LocalError::WrongPhase.into());
    }
    self.main.ensure(self.operation.reviewed())?;
    let actual = self.task_git()?.snapshot()?;
    self.operation.ensure_task_owned(&actual)?;
    let result = self.task_git()?.preview_task_abort(&actual)?;
    for file in &actual.files {
      if let Some(bytes) = self.task_git()?.file(&file.path)? {
        self.retain_result_bytes(&file.path, &bytes)?;
      }
    }
    self.operation.ensure_task_owned(&self.task_git()?.snapshot()?)?;
    self.operation.plan_task_abort(result)?;
    self.save()?;
    self.operation.resume_saved_phase();
    self.reconcile_task_abort()
  }

  pub(super) fn reconcile_task_abort(&mut self) -> Result<(), IntegrationError> {
    let before = self.operation.owned_task().ok_or(LocalError::Reconcile)?.clone();
    let result = self.operation.task_abort_result().ok_or(LocalError::Reconcile)?.clone();
    let actual = self.task_git()?.snapshot()?;
    if actual != result {
      if actual != before {
        return Err(LocalError::ForeignOperation.into());
      }
      self.main.ensure(self.operation.reviewed())?;
      self.task_git()?.abort_task_merge(&before)?;
    }
    let task = self.task_git()?.snapshot()?;
    self
      .operation
      .task_abort_finished(self.main.verified_snapshot()?, task)?;
    self.save()
  }
}
