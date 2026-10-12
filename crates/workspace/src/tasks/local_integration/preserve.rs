use orchester_core::integration::local::{LocalError, LocalPath, LocalRecoveryStore, RecoveryLayer};
use orchester_exec::local_git::digest;

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  pub fn selected_commit_tree(&self, paths: &[LocalPath]) -> Result<String, IntegrationError> {
    Ok(self.main.selected_tree(self.operation.reviewed(), paths)?)
  }

  pub fn commit(&mut self, paths: &[LocalPath], tree: &str, message: &str) -> Result<(), IntegrationError> {
    let current = self.main.snapshot()?;
    self.operation.begin_commit(&current, message)?;
    self.operation.commit_intent(paths.to_vec(), tree.to_owned())?;
    self.save()?;
    self.retain_commit_originals(&current)?;
    let result = self.main.commit_selected(&current, paths, tree, message);
    match result {
      Ok(current) => {
        self.operation.commit_finished(current)?;
        self.save()
      }
      Err(error) => {
        if self.main.snapshot()? == current {
          self.operation.commit_finished(current)?;
        } else {
          self.operation.needs_reconciliation();
        }
        self.save()?;
        Err(error.into())
      }
    }
  }

  pub fn preserve(&mut self, name: &str) -> Result<(), IntegrationError> {
    let current = self.main.snapshot()?;
    self.operation.begin_preserve(&current, name)?;
    self.save()?;
    self.complete_preservation(&current)
  }

  pub(super) fn complete_preservation(
    &mut self,
    current: &orchester_core::integration::local::Checkout,
  ) -> Result<(), IntegrationError> {
    self.main.ensure(current)?;
    for file in &current.files {
      for (layer, entry) in [(RecoveryLayer::Head, &file.head), (RecoveryLayer::Staged, &file.staged)] {
        if let Some(entry) = entry {
          if !matches!(entry.mode, 0o100644 | 0o100755) {
            return Err(LocalError::Unsupported.into());
          }
          let bytes = self.main.blob(&entry.object)?;
          self
            .db
            .retain_local_content(self.run.id(), &file.path, layer, &digest(&bytes), &bytes)?;
        }
      }
      if file.head.is_some() {
        let bytes = self.main.checkout_blob(&current.head, &file.path)?;
        self.db.retain_local_content(
          self.run.id(),
          &file.path,
          RecoveryLayer::WorkingBase,
          &digest(&bytes),
          &bytes,
        )?;
      }
      if let Some(bytes) = self.main.file(&file.path)? {
        if Some(digest(&bytes)) != file.working {
          return Err(LocalError::Stale.into());
        }
        self.db.retain_local_content(
          self.run.id(),
          &file.path,
          RecoveryLayer::Working,
          &digest(&bytes),
          &bytes,
        )?;
      }
    }
    self.main.ensure(current)?;
    self.operation.preservation_verified()?;
    self.save()?;
    let clean = self.main.clear_preserved(current)?;
    self.operation.prepared(clean)?;
    self.save()
  }

  pub fn prepare_if_clean(&mut self) -> Result<(), IntegrationError> {
    if self.operation.phase() == orchester_core::integration::local::LocalPhase::Review {
      self.prepare_clean()?;
    }
    Ok(())
  }

  pub fn prepare_clean(&mut self) -> Result<(), IntegrationError> {
    let current = self.main.snapshot()?;
    self.main.ensure(self.operation.reviewed())?;
    self.operation.prepared(current)?;
    self.save()
  }
}
impl LocalIntegration {
  fn retain_commit_originals(
    &mut self,
    reviewed: &orchester_core::integration::local::Checkout,
  ) -> Result<(), IntegrationError> {
    for file in &reviewed.files {
      if let Some(bytes) = self.main.file(&file.path)? {
        if Some(digest(&bytes)) != file.working {
          return Err(LocalError::Stale.into());
        }
        self.retain_result_bytes(&bytes)?;
      }
      for entry in [&file.head, &file.staged].into_iter().flatten() {
        let bytes = self.main.blob(&entry.object)?;
        self.retain_result_bytes(&bytes)?;
      }
    }
    self.main.ensure(reviewed)?;
    Ok(())
  }

  pub(super) fn retain_result_bytes(&mut self, bytes: &[u8]) -> Result<(), IntegrationError> {
    let hash = digest(bytes);
    let path = LocalPath::new(&format!("retained-results/{}", hash.as_str()))?;
    self
      .db
      .retain_local_content(self.run.id(), &path, RecoveryLayer::Result, &hash, bytes)?;
    Ok(())
  }
}
