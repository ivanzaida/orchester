use super::{LocalOperation, LocalPhase};
use crate::integration::local::{ContainedResult, LocalError, ResultEvidenceKind};

impl LocalOperation {
  /// Completes a record that holds nothing ([`Self::reserves_checkout`]) once `evidence` proves the run's whole
  /// work is in the checkout's branch, merged without Orchester: a review, also one paused since, or a cancellation
  /// whose kept originals were removed, then reads as complete instead of waiting for a merge that happened. No
  /// merge is recorded; [`Self::merged_outside`] keeps the commit that holds the work.
  pub fn settle_merged_outside(&mut self, evidence: &ContainedResult) -> Result<(), LocalError> {
    self.ensure_merged_outside(evidence)?;
    if self.reserves_checkout() {
      return Err(LocalError::WrongPhase);
    }
    self.merged_outside = Some(evidence.target.as_str().to_owned());
    self.checked = None;
    self.approved = None;
    self.paused_from = None;
    self.phase = LocalPhase::Complete;
    Ok(())
  }

  /// Records, on a person's decision, the merge `evidence` proves was made without Orchester as this record's
  /// completed merge, as a merge Orchester recorded is ([`Self::reconcile_recorded_merge`]): originals still
  /// retained are put back onto it next ([`LocalPhase::MergedAwaitingRestore`]), and a record that holds nothing
  /// else to finish completes. The task merge is never repeated.
  pub fn record_merged_outside(&mut self, evidence: &ContainedResult) -> Result<(), LocalError> {
    self.ensure_merged_outside(evidence)?;
    let target = evidence.target.as_str().to_owned();
    self.merged = Some(target.clone());
    self.merged_outside = Some(target);
    self.checked = None;
    self.approved = None;
    self.paused_from = None;
    self.phase = self.merged_phase();
    Ok(())
  }

  /// The commit of the checkout's branch that held the run's whole work when the record was found merged without
  /// Orchester ([`Self::settle_merged_outside`], [`Self::record_merged_outside`]).
  pub fn merged_outside(&self) -> Option<&str> {
    self.merged_outside.as_deref()
  }

  /// Only proven containment of the whole work counts, and only for a record that has no merge of its own, recorded
  /// or under way.
  fn ensure_merged_outside(&self, evidence: &ContainedResult) -> Result<(), LocalError> {
    if evidence.kind != ResultEvidenceKind::CompleteAncestry {
      return Err(LocalError::Reconcile);
    }
    if self.phase == LocalPhase::Complete || self.merged.is_some() || self.merge_pending() {
      return Err(LocalError::WrongPhase);
    }
    Ok(())
  }
}
