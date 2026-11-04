//! The approvals area of the local link: a run's rules and auto-approval, the project's rules and a run's tool
//! requests, read from the project database.

use std::ops::Deref;

use orchester_core::run::{ApprovalRule, PendingToolApproval, ProjectRule, review::Review};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  approvals::{
    ApprovalLink, ApprovalRequest, AutoApproveRequest, PendingApprovalsRequest, ProjectRulesRequest, RunRulesRequest,
  },
};

impl<D: Deref<Target = ProjectDb>> ApprovalLink for LocalLink<D> {
  fn approval_rules(&self, request: &RunRulesRequest) -> Result<Vec<ApprovalRule>, LinkError> {
    self.0.approval_rules(request.run).map_err(|error| read_failed(&error))
  }

  fn project_approval_rules(&self, _: &ProjectRulesRequest) -> Result<Vec<ProjectRule>, LinkError> {
    self.0.project_approval_rules().map_err(|error| read_failed(&error))
  }

  fn run_auto_approve(&self, request: &AutoApproveRequest) -> Result<Option<bool>, LinkError> {
    self
      .0
      .run_auto_approve(request.run)
      .map_err(|error| read_failed(&error))
  }

  fn pending_approvals(&self, request: &PendingApprovalsRequest) -> Result<Vec<PendingToolApproval>, LinkError> {
    self
      .0
      .pending_tool_approvals(request.run)
      .map_err(|error| read_failed(&error))
  }

  fn approval_answered(&self, request: &ApprovalRequest) -> Result<bool, LinkError> {
    self
      .0
      .tool_approval_answered(request.run, &request.id)
      .map_err(|error| read_failed(&error))
  }

  fn approval_review(&self, request: &ApprovalRequest) -> Result<Option<Review>, LinkError> {
    self
      .0
      .tool_approval_review(request.run, &request.id)
      .map_err(|error| read_failed(&error))
  }
}
