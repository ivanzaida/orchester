//! The approvals area of the local link: a run's rules and auto-approval, the project's rules and a run's tool
//! requests, read from the project database. In the runner's own tests each request and answer crosses as JSON
//! (`executor::link::sent`).

use std::ops::Deref;

use orchester_core::run::{ApprovalRule, PendingToolApproval, ProjectRule, review::Review};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
#[cfg(feature = "test-agent")]
use crate::execution::executor::link::sent;
use crate::execution::executor::link::{
  LinkError,
  approvals::{
    ApprovalLink, ApprovalRequest, AutoApproveRequest, PendingApprovalsRequest, ProjectRulesRequest, RunRulesRequest,
  },
};

/// `request` and the answer `read` gives it, each sent as JSON in the runner's own tests.
fn answered<R, T>(request: &R, read: impl FnOnce() -> Result<T, LinkError>) -> Result<T, LinkError>
where
  R: serde::Serialize + serde::de::DeserializeOwned,
  T: serde::Serialize + serde::de::DeserializeOwned,
{
  #[cfg(feature = "test-agent")]
  sent::sending(request);
  #[cfg(not(feature = "test-agent"))]
  let _ = request;
  let answer = read();
  #[cfg(feature = "test-agent")]
  sent::sending(&answer);
  answer
}

impl<D: Deref<Target = ProjectDb>> ApprovalLink for LocalLink<D> {
  fn approval_rules(&self, request: &RunRulesRequest) -> Result<Vec<ApprovalRule>, LinkError> {
    answered(request, || {
      self.0.approval_rules(request.run).map_err(|error| read_failed(&error))
    })
  }

  fn project_approval_rules(&self, request: &ProjectRulesRequest) -> Result<Vec<ProjectRule>, LinkError> {
    answered(request, || {
      self.0.project_approval_rules().map_err(|error| read_failed(&error))
    })
  }

  fn run_auto_approve(&self, request: &AutoApproveRequest) -> Result<Option<bool>, LinkError> {
    answered(request, || {
      self
        .0
        .run_auto_approve(request.run)
        .map_err(|error| read_failed(&error))
    })
  }

  fn pending_approvals(&self, request: &PendingApprovalsRequest) -> Result<Vec<PendingToolApproval>, LinkError> {
    answered(request, || {
      self
        .0
        .pending_tool_approvals(request.run)
        .map_err(|error| read_failed(&error))
    })
  }

  fn approval_answered(&self, request: &ApprovalRequest) -> Result<bool, LinkError> {
    answered(request, || {
      self
        .0
        .tool_approval_answered(request.run, &request.id)
        .map_err(|error| read_failed(&error))
    })
  }

  fn approval_review(&self, request: &ApprovalRequest) -> Result<Option<Review>, LinkError> {
    answered(request, || {
      self
        .0
        .tool_approval_review(request.run, &request.id)
        .map_err(|error| read_failed(&error))
    })
  }
}
