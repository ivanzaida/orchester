//! A single tool call waiting for a person's decision while its Codex process remains alive.
//! The record contains only display metadata; it never stores tool arguments or credentials.

use crate::{
  id::{InvocationId, RunId, StepExecutionId},
  time::Timestamp,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingToolApproval {
  pub id: String,
  pub run: RunId,
  pub execution: StepExecutionId,
  pub invocation: InvocationId,
  pub agent: String,
  pub server: String,
  pub action: String,
  pub target: Option<String>,
  pub requested_at: Timestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolApprovalDecision {
  Approved,
  Denied,
}

impl ToolApprovalDecision {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Approved => "approved",
      Self::Denied => "denied",
    }
  }
}
