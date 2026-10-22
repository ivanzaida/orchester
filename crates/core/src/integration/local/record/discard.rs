use super::{Checkout, LocalError, LocalOperation, LocalPhase};

impl LocalOperation {
  /// Discarding starts, like a commit or a preservation, only from a review of the checkout as it is now.
  pub fn begin_discard(&self, current: &Checkout) -> Result<(), LocalError> {
    self.prepare_phase(current)
  }

  /// The checkout as discarding left it, read again: a clean one is prepared for delivery, as after a commit or a
  /// preservation; one with changes left, discarded only in part or not at all, is reviewed again.
  pub fn discard_finished(&mut self, current: Checkout) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Review || self.merged.is_some() {
      return Err(LocalError::WrongPhase);
    }
    if current.operation.is_some() {
      return Err(LocalError::ForeignOperation);
    }
    self.reviewed = current;
    self.checked = None;
    self.approved = None;
    self.phase = if self.reviewed.clean() {
      LocalPhase::Prepared
    } else {
      LocalPhase::Review
    };
    Ok(())
  }
}
