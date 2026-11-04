//! The approvals area of the coordinator link: what Orchester answers a run's tool requests by, read again at each
//! request as before — the run's rules (*Approve similar*, a command *Always allowed* during the run), the project's
//! rules, the run's own auto-approval — and the requests themselves: those still waiting, whether one was answered,
//! and what auto mode did with one. Matching a request against the rules stays the executor's (`orchester_core::run`).

use orchester_core::{
  id::RunId,
  run::{ApprovalRule, PendingToolApproval, ProjectRule, review::Review},
};
use serde::{Deserialize, Serialize};

use super::LinkError;

/// The coordinator's answers about a run's tool requests and the rules that answer them.
pub trait ApprovalLink {
  /// The rules of the run `request` names, oldest first, revoked ones too.
  fn approval_rules(&self, request: &RunRulesRequest) -> Result<Vec<ApprovalRule>, LinkError>;

  /// The rules of the project, oldest first, revoked ones too.
  fn project_approval_rules(&self, request: &ProjectRulesRequest) -> Result<Vec<ProjectRule>, LinkError>;

  /// The run's own auto-approval choice; `None` follows each asking profile.
  fn run_auto_approve(&self, request: &AutoApproveRequest) -> Result<Option<bool>, LinkError>;

  /// The requests of the run `request` names that still wait, oldest first.
  fn pending_approvals(&self, request: &PendingApprovalsRequest) -> Result<Vec<PendingToolApproval>, LinkError>;

  /// Whether the request `request` names was answered, by a person or by Orchester; one that ended without an
  /// answer, or is unknown, was not.
  fn approval_answered(&self, request: &ApprovalRequest) -> Result<bool, LinkError>;

  /// What auto mode did with the request `request` names; `None` when it did not look at it.
  fn approval_review(&self, request: &ApprovalRequest) -> Result<Option<Review>, LinkError>;
}

/// A run's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRulesRequest {
  pub run: RunId,
}

/// The project's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRulesRequest;

/// A run's own auto-approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoApproveRequest {
  pub run: RunId,
}

/// A run's waiting requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApprovalsRequest {
  pub run: RunId,
}

/// One request of a run, by its id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
  pub run: RunId,
  pub id: String,
}

/// The rules of `run`, asked through `link`.
pub fn approval_rules(link: &(impl ApprovalLink + ?Sized), run: RunId) -> Result<Vec<ApprovalRule>, LinkError> {
  link.approval_rules(&RunRulesRequest { run })
}

/// The rules of the project, asked through `link`.
pub fn project_approval_rules(link: &(impl ApprovalLink + ?Sized)) -> Result<Vec<ProjectRule>, LinkError> {
  link.project_approval_rules(&ProjectRulesRequest)
}

/// `run`'s own auto-approval, asked through `link`.
pub fn run_auto_approve(link: &(impl ApprovalLink + ?Sized), run: RunId) -> Result<Option<bool>, LinkError> {
  link.run_auto_approve(&AutoApproveRequest { run })
}

/// The waiting requests of `run`, asked through `link`.
pub fn pending_approvals(
  link: &(impl ApprovalLink + ?Sized),
  run: RunId,
) -> Result<Vec<PendingToolApproval>, LinkError> {
  link.pending_approvals(&PendingApprovalsRequest { run })
}

/// Whether request `id` of `run` was answered, asked through `link`.
pub fn approval_answered(link: &(impl ApprovalLink + ?Sized), run: RunId, id: &str) -> Result<bool, LinkError> {
  link.approval_answered(&ApprovalRequest { run, id: id.to_owned() })
}

/// What auto mode did with request `id` of `run`, asked through `link`.
pub fn approval_review(link: &(impl ApprovalLink + ?Sized), run: RunId, id: &str) -> Result<Option<Review>, LinkError> {
  link.approval_review(&ApprovalRequest { run, id: id.to_owned() })
}
