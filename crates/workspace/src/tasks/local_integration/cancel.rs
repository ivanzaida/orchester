use orchester_core::integration::local::{LocalError, LocalPhase, LocalRecoveryStore, RecoveryLayer};

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  /// Restore preserved originals, or release completed preparation without changing either checkout.
  pub fn cancel_preservation(&mut self) -> Result<(), IntegrationError> {
    let actual = self.main.verified_snapshot()?;
    if self.operation.preservation().is_none() {
      if self.operation.owned_task().is_some() {
        let task = self.task_git()?.snapshot()?;
        self.operation.ensure_task_owned(&task)?;
        if !task.clean() {
          return Err(LocalError::ForeignOperation.into());
        }
      }
      self.operation.begin_cancel(&actual)?;
      self.operation.cancellation_verified()?;
      return self.save();
    }
    if self.operation.phase() != LocalPhase::Canceling {
      self.operation.begin_cancel(&actual)?;
      self.save()?;
    } else if actual.head != self.operation.reviewed().head || actual.operation.is_some() {
      return Err(LocalError::Stale.into());
    }
    let original = self
      .operation
      .preservation()
      .ok_or(LocalError::NotVerified)?
      .original
      .clone();
    self.resume_restored_writes()?;
    for file in &original.files {
      if self.operation.restored().iter().any(|done| done.path == file.path) {
        continue;
      }
      let bytes = self
        .db
        .local_content(self.run.id(), &file.path, RecoveryLayer::Working)?;
      let expected = orchester_exec::local_git::file_hash(self.main.root(), &file.path)?;
      let heads = self.main.tree_entries("HEAD")?;
      let baseline = heads
        .get(&file.path)
        .map(|_| self.main.checkout_blob(&actual.head, &file.path))
        .transpose()?;
      if expected != baseline.as_deref().map(orchester_exec::local_git::digest) {
        return Err(LocalError::Stale.into());
      }
      self.write_restored(
        &file.path,
        expected.as_ref(),
        bytes.as_deref(),
        file.staged.clone(),
        file.executable,
      )?;
    }
    self.verify_restored()?;
    self.operation.cancellation_verified()?;
    self.save()
  }
}
