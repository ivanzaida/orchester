use orchester_core::integration::local::{
  IndexEntry, LocalConflict, LocalError, LocalFile, LocalPath, LocalRecoveryStore, Placement, RecoveryLayer,
  Resolution, RestoredFile,
};
use orchester_exec::local_git::{digest, executable, file_hash};

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  pub fn restore(&mut self) -> Result<(), IntegrationError> {
    self.ensure_merged_checkout()?;
    self.operation.begin_restore()?;
    self.save()?;
    let original = self
      .operation
      .preservation()
      .ok_or(LocalError::NotVerified)?
      .original
      .clone();
    self.resume_restored_writes()?;
    let mut conflicts = false;
    for file in &original.files {
      if self
        .operation
        .restored()
        .iter()
        .any(|restored| restored.path == file.path)
      {
        continue;
      }
      let (working, staged) = match self.restore_result(file)? {
        Some(result) => result,
        None => {
          conflicts = true;
          continue;
        }
      };
      let expected = file_hash(self.main.root(), &file.path)?;
      self.write_restored(
        &file.path,
        expected.as_ref(),
        working.as_deref(),
        staged,
        self.restored_mode(file)?,
      )?;
    }
    if conflicts {
      self.operation.restoration_conflicts();
      return self.save();
    }
    self.finish_restore()
  }

  fn restored_mode(&self, file: &LocalFile) -> Result<bool, IntegrationError> {
    let base = file.head.as_ref().is_some_and(|entry| entry.mode == 0o100755);
    if file.executable != base {
      return Ok(file.executable);
    }
    let current = self.main.tree_entries("HEAD")?;
    Ok(current.get(&file.path).is_some_and(|entry| entry.mode == 0o100755))
  }

  fn restore_result(&self, file: &LocalFile) -> Result<RestoreResult, IntegrationError> {
    let heads = self.main.tree_entries("HEAD")?;
    let current_entry = heads.get(&file.path);
    let current = self.main.file(&file.path)?;
    if self.main.index_entries()?.get(&file.path) != current_entry {
      return Err(LocalError::Stale.into());
    }
    if file.head.is_none() && current.is_some() {
      return Ok(None);
    }
    // An externally changed file cannot be adopted as the merged checkout version.
    let merged_head = self.operation.merged().ok_or(LocalError::WrongPhase)?;
    let merged_working = current_entry
      .map(|_| self.main.checkout_blob(merged_head, &file.path))
      .transpose()?;
    if current.as_deref().map(digest) != merged_working.as_deref().map(digest) {
      return Err(LocalError::Stale.into());
    }
    let base = self.db.local_content(self.run.id(), &file.path, RecoveryLayer::Head)?;
    let working_base = self
      .db
      .local_content(self.run.id(), &file.path, RecoveryLayer::WorkingBase)?;
    let local = self
      .db
      .local_content(self.run.id(), &file.path, RecoveryLayer::Working)?;
    let working = self.merge_optional(working_base.as_deref(), current.as_deref(), local.as_deref())?;
    let Some(working) = working else {
      return Ok(None);
    };
    let staged = if file.staged == file.head {
      current_entry.cloned()
    } else {
      let incoming = self
        .db
        .local_content(self.run.id(), &file.path, RecoveryLayer::Staged)?;
      let merged_index = current_entry.map(|entry| self.main.blob(&entry.object)).transpose()?;
      let staged = self.merge_optional(base.as_deref(), merged_index.as_deref(), incoming.as_deref())?;
      let Some(staged) = staged else {
        return Ok(None);
      };
      staged
        .as_deref()
        .map(|bytes| {
          self.main.put_blob(bytes).map(|object| IndexEntry {
            mode: file.staged.as_ref().map_or(0o100644, |entry| entry.mode),
            object,
          })
        })
        .transpose()?
    };
    Ok(Some((working, staged)))
  }

  fn merge_optional(
    &self,
    base: Option<&[u8]>,
    current: Option<&[u8]>,
    incoming: Option<&[u8]>,
  ) -> Result<Option<Option<Vec<u8>>>, IntegrationError> {
    if incoming == base {
      return Ok(Some(current.map(<[u8]>::to_vec)));
    }
    if current == base || current == incoming {
      return Ok(Some(incoming.map(<[u8]>::to_vec)));
    }
    match (base, current, incoming) {
      (Some(base), Some(current), Some(incoming)) => Ok(self.main.merge_bytes(base, current, incoming)?.map(Some)),
      _ => Ok(None),
    }
  }

  pub fn restore_conflicts(&self) -> Result<Vec<LocalConflict>, IntegrationError> {
    restoration_files(&self.db, &self.operation, &self.main)
  }

  pub fn save_restore_resolution(
    &mut self,
    reviewed: &LocalConflict,
    expected: Option<&orchester_core::blob::ContentHash>,
    resolution: &Resolution,
    placement: Placement,
  ) -> Result<(), IntegrationError> {
    self.ensure_merged_checkout()?;
    if !self.restore_conflicts()?.contains(reviewed) {
      return Err(LocalError::Stale.into());
    }
    let (content, entry) = self.main.resolution_bytes(reviewed, resolution)?;
    let executable = entry.as_ref().is_some_and(|entry| entry.mode == 0o100755);
    if let Resolution::KeepBoth(destination) = resolution {
      if self.main.file(destination)?.is_some() || self.main.index_entries()?.contains_key(destination) {
        return Err(LocalError::Collision.into());
      }
      let incoming = reviewed.incoming.as_ref().ok_or(LocalError::Unsupported)?;
      let bytes = self.main.blob(&incoming.object)?;
      let staged = self.placement_entry(destination, Some(incoming.clone()), placement)?;
      self.plan_restored(destination, None, Some(&bytes), staged, incoming.mode == 0o100755)?;
    }
    let destination = match resolution {
      Resolution::AtPath { destination, .. } => destination,
      _ => &reviewed.path,
    };
    if destination != &reviewed.path {
      if self.main.file(destination)?.is_some() || self.main.index_entries()?.contains_key(destination) {
        return Err(LocalError::Collision.into());
      }
      let staged = self.placement_entry(destination, entry, placement)?;
      self.plan_restored(destination, None, content.as_deref(), staged, executable)?;
      self.plan_restored(&reviewed.path, expected, None, None, false)?;
    } else {
      let staged = self.placement_entry(destination, entry, placement)?;
      self.plan_restored(destination, expected, content.as_deref(), staged, executable)?;
    }
    self.operation.restoration_conflicts();
    self.save()?;
    self.resume_restored_writes()?;
    self.verify_restored()
  }

  fn placement_entry(
    &self,
    path: &LocalPath,
    chosen: Option<IndexEntry>,
    placement: Placement,
  ) -> Result<Option<IndexEntry>, IntegrationError> {
    Ok(match placement {
      Placement::Stage => chosen,
      Placement::LeaveUnstaged => self.main.tree_entries("HEAD")?.get(path).cloned(),
    })
  }

  fn plan_restored(
    &mut self,
    path: &LocalPath,
    expected: Option<&orchester_core::blob::ContentHash>,
    working: Option<&[u8]>,
    staged: Option<IndexEntry>,
    executable: bool,
  ) -> Result<(), IntegrationError> {
    if file_hash(self.main.root(), path)?.as_ref() != expected {
      return Err(LocalError::Stale.into());
    }
    let executable = cfg!(unix) && executable;
    if let Some(bytes) = working {
      self.retain_result_bytes(path, bytes)?;
    }
    let heads = self.main.tree_entries("HEAD")?;
    if self.main.index_entries()?.get(path) != heads.get(path) {
      return Err(LocalError::Stale.into());
    }
    // Record the intended result before writing it. Restart verification can distinguish a completed write.
    self.operation.restored_file(RestoredFile {
      path: path.clone(),
      before: expected.cloned(),
      executable,
      working: working.map(digest),
      staged,
    })?;
    Ok(())
  }

  pub(super) fn write_restored(
    &mut self,
    path: &LocalPath,
    expected: Option<&orchester_core::blob::ContentHash>,
    working: Option<&[u8]>,
    staged: Option<IndexEntry>,
    executable: bool,
  ) -> Result<(), IntegrationError> {
    self.plan_restored(path, expected, working, staged, executable)?;
    self.save()?;
    self.resume_restored_writes()?;
    self.verify_restored()
  }

  pub fn finish_restore(&mut self) -> Result<(), IntegrationError> {
    self.ensure_merged_checkout()?;
    self.verify_restored()?;
    let original = &self.operation.preservation().ok_or(LocalError::NotVerified)?.original;
    if original
      .files
      .iter()
      .any(|file| !self.operation.restored().iter().any(|done| done.path == file.path))
    {
      return Err(LocalError::Unresolved.into());
    }
    self.operation.restoration_verified()?;
    self.save()
  }

  pub(super) fn verify_restored(&self) -> Result<(), IntegrationError> {
    if !changed_since_restoration(&self.main, &self.operation)?.is_empty() {
      return Err(LocalError::Reconcile.into());
    }
    Ok(())
  }

  /// An interrupted cleanup goes on only while the restored files differ exactly where the person accepted; any
  /// other change keeps the originals.
  pub(super) fn verify_accepted_edits(&self) -> Result<(), IntegrationError> {
    if changed_since_restoration(&self.main, &self.operation)? != self.operation.accepted_edits() {
      return Err(LocalError::Reconcile.into());
    }
    Ok(())
  }

  fn ensure_merged_checkout(&self) -> Result<(), IntegrationError> {
    let actual = self.main.verified_snapshot()?;
    if Some(actual.head.as_str()) != self.operation.merged() || actual.operation.is_some() {
      return Err(LocalError::Stale.into());
    }
    Ok(())
  }

  pub fn remove_preservation(&mut self) -> Result<(), IntegrationError> {
    self.remove_preservation_accepting(&[])
  }

  /// Removes the retained originals after a verified restoration or cancellation. `accepted` are the restored files
  /// the person changed since and saw listed: they stay as they are. Any other difference keeps the originals.
  pub fn remove_preservation_accepting(&mut self, accepted: &[LocalPath]) -> Result<(), IntegrationError> {
    if self.operation.merged().is_some() {
      self.ensure_merged_checkout()?;
    } else {
      let actual = self.main.verified_snapshot()?;
      if actual.head != self.operation.reviewed().head || actual.operation.is_some() {
        return Err(LocalError::Stale.into());
      }
    }
    if changed_since_restoration(&self.main, &self.operation)? != accepted {
      return Err(LocalError::Stale.into());
    }
    self.operation.begin_cleanup_accepting(accepted.to_vec())?;
    self.save()?;
    self.db.remove_local_content(self.run.id())?;
    self.operation.cleanup_finished()?;
    self.save()
  }
}

type RestoreResult = Option<(Option<Vec<u8>>, Option<IndexEntry>)>;
pub(super) fn restoration_files(
  db: &orchester_store::ProjectDb,
  operation: &orchester_core::integration::local::LocalOperation,
  main: &orchester_exec::local_git::LocalGit,
) -> Result<Vec<LocalConflict>, IntegrationError> {
  let Some(saved) = operation.preservation() else {
    return Ok(Vec::new());
  };
  let heads = main.tree_entries("HEAD")?;
  saved
    .original
    .files
    .iter()
    .filter(|file| !operation.restored().iter().any(|done| done.path == file.path))
    .map(|file| {
      let local = db.local_content(operation.run(), &file.path, RecoveryLayer::Working)?;
      let incoming = local
        .as_deref()
        .map(|bytes| {
          main.put_blob(bytes).map(|object| IndexEntry {
            mode: if file.executable { 0o100755 } else { 0o100644 },
            object,
          })
        })
        .transpose()?;
      Ok(LocalConflict {
        path: file.path.clone(),
        ancestor: file.head.clone(),
        current: heads.get(&file.path).cloned(),
        incoming,
      })
    })
    .collect()
}

/// The restored paths whose working content, staging or executable bit changed since restoration verified them.
pub(super) fn changed_since_restoration(
  main: &orchester_exec::local_git::LocalGit,
  operation: &orchester_core::integration::local::LocalOperation,
) -> Result<Vec<orchester_core::integration::local::LocalPath>, IntegrationError> {
  let index = main.index_entries()?;
  let mut changed = Vec::new();
  for restored in operation.restored() {
    if file_hash(main.root(), &restored.path)? != restored.working
      || index.get(&restored.path) != restored.staged.as_ref()
      || (restored.working.is_some() && executable(main.root(), &restored.path)? != restored.executable)
    {
      changed.push(restored.path.clone());
    }
  }
  Ok(changed)
}
