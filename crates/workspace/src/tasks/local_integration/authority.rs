//! Calling origin is retained by the caller; a data endpoint cannot create a reviewed native decision.
use orchester_core::integration::local::LocalError;

use super::{IntegrationError, LocalIntegration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalOrigin {
  Native,
  WindowAutomation,
  AuthenticatedDomain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalIntent {
  Candidate,
  ReviewedDecision,
}

impl LocalOrigin {
  pub fn authorize(self, intent: LocalIntent) -> Result<(), LocalError> {
    match (self, intent) {
      (Self::Native, _) | (Self::WindowAutomation, LocalIntent::Candidate) => Ok(()),
      _ => Err(LocalError::ApprovalRequired),
    }
  }
}

pub type LocalAuthorization = std::sync::Arc<dyn Fn() -> Result<(), LocalError> + Send + Sync>;

impl LocalIntegration {
  pub(super) fn ensure_authorized(&self) -> Result<(), IntegrationError> {
    if let Some(authorize) = &self.authorization {
      authorize()?;
    }
    Ok(())
  }
  /// The native worker and managed domain route use the same authority and generation boundary.
  pub fn authorize(
    &self,
    origin: LocalOrigin,
    intent: LocalIntent,
    generation: Option<u64>,
  ) -> Result<(), IntegrationError> {
    origin.authorize(intent)?;
    if generation.is_some_and(|generation| self.operation.version() != generation) {
      return Err(LocalError::Stale.into());
    }
    Ok(())
  }
}
