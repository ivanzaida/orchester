use super::{LocalError, LocalOperation, LocalPhase};
use crate::integration::local::{ConflictKind, DraftRecord};

impl LocalOperation {
  pub fn editor_drafts(&self) -> &[DraftRecord] {
    &self.editor_drafts
  }

  pub fn save_editor_draft(&mut self, draft: DraftRecord) -> Result<(), LocalError> {
    let phase = match draft.identity.kind {
      ConflictKind::Task => LocalPhase::TaskConflicts,
      ConflictKind::Restoration => LocalPhase::RestoreConflicts,
    };
    if self.phase != phase {
      return Err(LocalError::WrongPhase);
    }
    self.editor_drafts.retain(|saved| saved.identity != draft.identity);
    self.editor_drafts.push(draft);
    Ok(())
  }
}
