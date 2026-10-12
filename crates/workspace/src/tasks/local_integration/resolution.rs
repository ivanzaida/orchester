use orchester_core::integration::local::{
  LocalConflict, LocalError, LocalPath, LocalRecoveryStore, PendingResolution, RecoveryLayer, Resolution,
  ResolutionWrite,
};
use orchester_exec::local_git::{digest, file_hash, set_executable, write_regular};

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  pub(super) fn plan_task_resolution(
    &mut self,
    file: &LocalConflict,
    expected: Option<&orchester_core::blob::ContentHash>,
    resolution: &Resolution,
  ) -> Result<(), IntegrationError> {
    let before = self.task_git()?.snapshot()?;
    self.operation.ensure_task_owned(&before)?;
    if file_hash(self.task_git()?.root(), &file.path)?.as_ref() != expected
      || !self.task_git()?.conflicts()?.contains(file)
    {
      return Err(LocalError::Stale.into());
    }
    let (bytes, entry) = self.task_git()?.resolution_bytes(file, resolution)?;
    let mut writes = Vec::new();
    match resolution {
      Resolution::KeepBoth(destination) => {
        let incoming = file.incoming.as_ref().ok_or(LocalError::Unsupported)?.clone();
        let incoming_bytes = self.task_git()?.blob(&incoming.object)?;
        self.ensure_destination(destination)?;
        writes.push(self.retain_write(destination, None, Some(&incoming_bytes), Some(incoming))?);
      }
      Resolution::AtPath { destination, .. } if destination != &file.path => {
        self.ensure_destination(destination)?;
        writes.push(self.retain_write(destination, None, bytes.as_deref(), entry.clone())?);
        writes.push(self.retain_write(&file.path, expected.cloned(), None, None)?);
      }
      _ => {}
    }
    if !matches!(resolution, Resolution::AtPath { destination, .. } if destination != &file.path) {
      writes.push(self.retain_write(&file.path, expected.cloned(), bytes.as_deref(), entry)?);
    }
    let untouched_index = self.untouched_index(&writes)?;
    self.operation.plan_resolution(PendingResolution {
      before,
      conflict: file.clone(),
      untouched_index,
      writes,
    })?;
    self.save()
  }

  fn ensure_destination(&self, destination: &LocalPath) -> Result<(), IntegrationError> {
    if self.task_git()?.file(destination)?.is_some()
      || self.task_git()?.index_entries()?.contains_key(destination)
      || self
        .task_git()?
        .conflicts()?
        .iter()
        .any(|file| &file.path == destination)
    {
      return Err(LocalError::Collision.into());
    }
    Ok(())
  }

  fn retain_write(
    &mut self,
    path: &LocalPath,
    before: Option<orchester_core::blob::ContentHash>,
    bytes: Option<&[u8]>,
    entry: Option<orchester_core::integration::local::IndexEntry>,
  ) -> Result<ResolutionWrite, IntegrationError> {
    let working = bytes.map(digest);
    if let Some((bytes, hash)) = bytes.zip(working.as_ref()) {
      self
        .db
        .retain_local_content(self.run.id(), &result_key(hash)?, RecoveryLayer::Result, hash, bytes)?;
    }
    Ok(ResolutionWrite {
      path: path.clone(),
      before,
      working,
      entry,
    })
  }

  fn untouched_index(&self, writes: &[ResolutionWrite]) -> Result<orchester_core::blob::ContentHash, IntegrationError> {
    let mut entries = self.task_git()?.index_entries()?;
    entries.retain(|path, _| !writes.iter().any(|write| &write.path == path));
    let encoded = serde_json::to_vec(&entries).map_err(|_| LocalError::Reconcile)?;
    Ok(digest(&encoded))
  }

  /// Complete only recorded before/after states. Unknown edits remain untouched and require review.
  pub(super) fn apply_pending_resolution(&mut self) -> Result<(), IntegrationError> {
    let plan = self
      .operation
      .pending_resolution()
      .ok_or(LocalError::Reconcile)?
      .clone();
    let mut before = plan.before.clone();
    let mut actual = self.task_git()?.snapshot()?;
    before
      .files
      .retain(|file| !plan.writes.iter().any(|write| write.path == file.path));
    actual
      .files
      .retain(|file| !plan.writes.iter().any(|write| write.path == file.path));
    before.index = actual.index.clone();
    if before != actual || self.untouched_index(&plan.writes)? != plan.untouched_index {
      return Err(LocalError::Stale.into());
    }
    for write in &plan.writes {
      self.apply_resolution_write(&plan, write)?;
    }
    self.operation.resolution_finished(self.task_git()?.snapshot()?)?;
    self.save()
  }

  fn apply_resolution_write(&self, plan: &PendingResolution, write: &ResolutionWrite) -> Result<(), IntegrationError> {
    let git = self.task_git()?;
    let actual = file_hash(git.root(), &write.path)?;
    let conflicts = git.conflicts()?;
    let entries = git.index_entries()?;
    let is_original = conflicts.contains(&plan.conflict) && write.path == plan.conflict.path;
    if actual != write.before && actual != write.working {
      return Err(LocalError::Stale.into());
    }
    if !is_original && entries.get(&write.path) != write.entry.as_ref() {
      // A newly created destination can still be waiting for its index write.
      if write.path == plan.conflict.path || entries.contains_key(&write.path) {
        return Err(LocalError::Stale.into());
      }
    }
    let bytes = write
      .working
      .as_ref()
      .map(|hash| {
        self
          .db
          .local_content(self.run.id(), &result_key(hash)?, RecoveryLayer::Result)
          .map_err(IntegrationError::from)?
          .ok_or(IntegrationError::State(LocalError::Reconcile))
      })
      .transpose()?;
    if actual != write.working {
      write_regular(git.root(), &write.path, actual.as_ref(), bytes.as_deref())?;
    }
    if let Some(hash) = &write.working {
      set_executable(
        git.root(),
        &write.path,
        hash,
        write.entry.as_ref().is_some_and(|entry| entry.mode == 0o100755),
      )?;
    }
    git.set_index(&write.path, write.entry.as_ref(), None)?;
    Ok(())
  }
}

fn result_key(hash: &orchester_core::blob::ContentHash) -> Result<LocalPath, LocalError> {
  LocalPath::new(&format!("retained-results/{}", hash.as_str()))
}
