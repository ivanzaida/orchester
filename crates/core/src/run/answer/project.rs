//! Rules of the project (the person's decision of 2026-10-18, "Approval choices and waiting requests"): *Approve for
//! every run of this project* on a request of Orchester's `start_qa_app` approves it, and every later call of it in any
//! run of the project, while the worktree's QA script is the one approved, by its git blob. A changed script asks
//! again, and so does a revoked rule. No other request is offered it: `start_qa_app` runs a project program outside
//! every sandbox, which the hard rules keep from auto mode's reviewer
//! ([`crate::run::review::ReviewHold::ProjectProgram`]), so without it a person approves it again in every run. Only
//! a person makes such a rule ([`super::ApprovalAnswer::given_by`]); it lives with the project, not with the run that
//! made it, and a person can revoke it.

use serde::{Deserialize, Serialize};

use super::{
  rule::RuleScope,
  subject::{ApprovalSubject, ToolReach},
};
use crate::{
  id::RunId,
  orchester_tools::{OrchesterTool, QaApproval, SERVER},
  run::PendingToolApproval,
  time::Timestamp,
};

/// A project rule's number, as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRuleId(i64);

impl ProjectRuleId {
  pub const fn new(value: i64) -> Self {
    Self(value)
  }

  pub const fn get(self) -> i64 {
    self.0
  }
}

impl RuleScope {
  /// The rule *Approve for every run of this project* makes of `subject`: Orchester's `start_qa_app` with the QA
  /// script's git blob, which the rule is kept to; `None` for any other request, and for a call that names no script.
  pub fn project(subject: &ApprovalSubject) -> Option<Self> {
    match subject {
      ApprovalSubject::McpTool {
        server,
        tool,
        reach: ToolReach::Script { blob },
      } if server == SERVER && tool == OrchesterTool::StartQaApp.as_str() && !blob.is_empty() => {
        Some(Self::SimilarMcpTool {
          server: server.clone(),
          tool: tool.clone(),
          folder: None,
          check: Some(blob.clone()),
        })
      }
      _ => None,
    }
  }

  /// [`Self::project`] of `approval`, read from the request alone: an MCP call's server, tool and target tell it,
  /// with no worktree to place a command in.
  pub fn project_of(approval: &PendingToolApproval) -> Option<Self> {
    if approval.command {
      return None;
    }
    let blob = QaApproval::script_of(approval.target.as_deref()?)?;
    Self::project(&ApprovalSubject::McpTool {
      server: approval.server.clone(),
      tool: approval.action.clone(),
      reach: ToolReach::Script { blob: blob.to_owned() },
    })
  }

  /// The QA script's git blob a project rule's scope keeps it to.
  pub fn script(&self) -> Option<&str> {
    match self {
      Self::SimilarMcpTool { check, .. } => check.as_deref(),
      Self::SimilarCommand { .. } | Self::AlwaysCommand { .. } => None,
    }
  }
}

/// A rule of the project ([`RuleScope::project`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRule {
  pub id: ProjectRuleId,
  pub scope: RuleScope,
  /// The run whose request a person approved with it.
  pub made_in: RunId,
  pub created_at: Timestamp,
  /// When a person revoked it; it answers nothing since.
  pub revoked_at: Option<Timestamp>,
  /// Requests it answered.
  pub answered: u32,
}

impl ProjectRule {
  pub const fn is_active(&self) -> bool {
    self.revoked_at.is_none()
  }

  /// The QA script's git blob the rule is kept to.
  pub fn script(&self) -> Option<&str> {
    self.scope.script()
  }
}

/// The active project rule of `rules` that answers `subject`: one made of the same tool with the same script.
pub fn answering_in_project<'a>(rules: &'a [ProjectRule], subject: &ApprovalSubject) -> Option<&'a ProjectRule> {
  let scope = RuleScope::project(subject)?;
  rules.iter().find(|rule| rule.is_active() && rule.scope == scope)
}
