use super::{LocalOperation, Preservation};

impl std::fmt::Debug for Preservation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Preservation")
      .field("verified", &self.verified)
      .field("retained", &self.retained)
      .field("original", &"<private>")
      .finish()
  }
}

impl std::fmt::Debug for LocalOperation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LocalOperation")
      .field("run", &self.run)
      .field("version", &self.version)
      .field("phase", &self.phase)
      .field("merged", &self.merged)
      .field("merged_outside", &self.merged_outside)
      .field("checkout_and_recovery", &"<private>")
      .finish()
  }
}
