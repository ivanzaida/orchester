//! Consent for decisions made on the person's behalf, shared by window input and domain tools.
use orchester_store::StoreError;

use crate::{app_mcp::ToolError, messages::Message, workspace::Workspace};

/// The decision being taken, independent of the transport that delivers it. No approval mode (Autopilot, Manual's
/// reviewer) takes any of them: each is the person's unless they let automation answer approvals
/// (`docs/design/approval-modes.md`, "Decisions").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
  Approval,
  Question,
  Plan,
  /// A run that waits for a person: a retry, a return to an earlier step, accepting a step as it is, an agent's
  /// resolution of a merge's conflicts, or a step completed anyway.
  Stuck,
  /// A run stopped at its token budget, time limit or a step's budget, continued with a raised or removed limit.
  Limit,
}

impl DecisionKind {
  /// All answers on the person's behalf require the approvals opt-in. Permission setup has a separate grant.
  pub const fn allowed(self, approvals: bool) -> bool {
    match self {
      Self::Approval | Self::Question | Self::Plan | Self::Stuck | Self::Limit => approvals,
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
    Err(not_allowed())
  }
}

/// A decision automation took without the person's consent, refused.
pub(crate) fn not_allowed() -> ToolError {
  ToolError::refused(
    "automation_not_allowed",
    vec![Message::new("appMcp.errors.automationNotAllowed")],
  )
}
