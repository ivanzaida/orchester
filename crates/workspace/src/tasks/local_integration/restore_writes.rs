use orchester_core::integration::local::{LocalError, LocalRecoveryStore, RecoveryLayer};
use orchester_exec::local_git::{file_hash, set_executable, write_regular};

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  /// Replays only recorded before/after bytes, including a person's edited restoration result.
  pub(super) fn resume_restored_writes(&self) -> Result<(), IntegrationError> {
    let heads = self.main.tree_entries("HEAD")?;
    for saved in self.operation.restored() {
      let working = file_hash(self.main.root(), &saved.path)?;
      let entries = self.main.index_entries()?;
      if working != saved.working && working != saved.before {
        return Err(LocalError::Stale.into());
      }
      if entries.get(&saved.path) != saved.staged.as_ref() && entries.get(&saved.path) != heads.get(&saved.path) {
        return Err(LocalError::Stale.into());
      }
      if working != saved.working {
        let bytes = saved
          .working
          .as_ref()
          .map(|hash| {
            let key = super::preserve::result_key(&saved.path, hash)?;
            self
              .db
              .local_content(self.run.id(), &key, RecoveryLayer::Result)?
              .ok_or(IntegrationError::State(LocalError::Reconcile))
          })
          .transpose()?;
        write_regular(self.main.root(), &saved.path, working.as_ref(), bytes.as_deref())?;
      }
      if let Some(hash) = &saved.working {
        set_executable(self.main.root(), &saved.path, hash, saved.executable)?;
      }
      self.main.set_index(&saved.path, saved.staged.as_ref(), None)?;
    }
    Ok(())
  }
}
