//! Consent for decisions made on the person's behalf, shared by window input and domain tools.
use orchester_store::StoreError;

use crate::{app_mcp::ToolError, messages::Message, workspace::Workspace};

/// The decision being taken, independent of the transport that delivers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
  Approval,
  Question,
  Plan,
  Stuck,
}

impl DecisionKind {
  /// All answers on the person's behalf require the approvals opt-in. Permission setup has a separate grant.
  pub const fn allowed(self, approvals: bool) -> bool {
    match self {
      Self::Approval | Self::Question | Self::Plan | Self::Stuck => approvals,
    }
  }

  /// Last verified consent, safe to read while delivering window input; unknown or revoked consent refuses.
  pub fn consent(self, workspace: &Workspace) -> bool {
    self.allowed(workspace.automation_approval_consent())
  }

  /// Reads stored consent off the UI thread before a domain tool decides.
  pub fn read_consent(self, workspace: &Workspace) -> Result<bool, StoreError> {
    workspace
      .automation_answers_approvals()
      .map(|allowed| self.allowed(allowed))
  }

  pub fn require(self, workspace: &Workspace) -> Result<(), ToolError> {
    if self.read_consent(workspace)? {
      return Ok(());
    }
    Err(ToolError::refused(
      "automation_not_allowed",
      vec![Message::new("appMcp.errors.automationNotAllowed")],
    ))
  }
}
