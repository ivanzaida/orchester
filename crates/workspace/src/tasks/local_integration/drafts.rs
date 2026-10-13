use orchester_core::{
  blob::ContentHash,
  integration::local::{
    ConflictKind, DraftIdentity, DraftRecord, EditorCandidate, LocalConflict, LocalError, LocalPath, LocalPhase,
    LocalRecoveryStore, RecoveryLayer,
  },
};
use orchester_exec::local_git::digest;

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  /// Save candidates only: neither Git file bytes nor conflict index stages are changed.
  pub fn save_editor_draft(
    &mut self,
    file: &LocalConflict,
    expected: Option<&ContentHash>,
    candidate: &EditorCandidate,
    generation: u64,
  ) -> Result<(), IntegrationError> {
    if self.operation.version() != generation {
      return Err(LocalError::Stale.into());
    }
    let identity = self.draft_identity(file, expected)?;
    let bytes = serde_json::to_vec(candidate).map_err(|_| LocalError::Unsupported)?;
    if bytes.len() > 16 * 1024 * 1024 {
      return Err(LocalError::Unsupported.into());
    }
    let hash = digest(&bytes);
    let path = LocalPath::new(&format!("editor-drafts/{}", hash.as_str()))?;
    self.ensure_authorized()?;
    self
      .db
      .retain_local_content(self.run.id(), &path, RecoveryLayer::EditorDraft, &hash, &bytes)?;
    if self.draft_identity(file, expected)? != identity {
      return Err(LocalError::Stale.into());
    }
    self.operation.save_editor_draft(DraftRecord {
      identity,
      content: hash,
    })?;
    self.save()
  }

  pub fn editor_draft(
    &self,
    file: &LocalConflict,
    expected: Option<&ContentHash>,
  ) -> Result<Option<EditorCandidate>, IntegrationError> {
    let identity = self.draft_identity(file, expected)?;
    let Some(saved) = self
      .operation
      .editor_drafts()
      .iter()
      .find(|saved| saved.identity == identity)
    else {
      return Ok(None);
    };
    read_candidate(&self.db, self.run.id(), saved).map(Some)
  }

  fn draft_identity(
    &self,
    file: &LocalConflict,
    expected: Option<&ContentHash>,
  ) -> Result<DraftIdentity, IntegrationError> {
    let main = self.main.snapshot()?;
    let (kind, task, current) = match self.operation.phase() {
      LocalPhase::TaskConflicts => {
        let task = self.task_git()?.snapshot()?;
        self.operation.ensure_task_owned(&task)?;
        (ConflictKind::Task, Some(task), self.task_git()?.conflicts()?)
      }
      LocalPhase::RestoreConflicts => {
        if Some(main.head.as_str()) != self.operation.merged() || main.operation.is_some() {
          return Err(LocalError::Stale.into());
        }
        (ConflictKind::Restoration, None, self.restore_conflicts()?)
      }
      _ => return Err(LocalError::WrongPhase.into()),
    };
    if !current.iter().any(|actual| actual == file) {
      return Err(LocalError::Stale.into());
    }
    let git = if kind == ConflictKind::Task {
      self.task_git()?
    } else {
      &self.main
    };
    if git.file(&file.path)?.as_deref().map(digest).as_ref() != expected {
      return Err(LocalError::Stale.into());
    }
    Ok(DraftIdentity {
      kind,
      main,
      task,
      file: file.clone(),
      expected: expected.cloned(),
    })
  }
}

pub(super) fn read_candidate(
  db: &orchester_store::ProjectDb,
  run: orchester_core::id::RunId,
  saved: &DraftRecord,
) -> Result<EditorCandidate, IntegrationError> {
  let path = LocalPath::new(&format!("editor-drafts/{}", saved.content.as_str()))?;
  let bytes = db
    .local_content(run, &path, RecoveryLayer::EditorDraft)?
    .ok_or(LocalError::NotVerified)?;
  if digest(&bytes) != saved.content {
    return Err(LocalError::NotVerified.into());
  }
  serde_json::from_slice(&bytes).map_err(|_| LocalError::NotVerified.into())
}
