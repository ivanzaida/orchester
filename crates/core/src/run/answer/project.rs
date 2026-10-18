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

/// What a rule of the project approves: Orchester's `start_qa_app`, while the QA script is at one git blob. Stored as
/// the [`RuleScope`] Approve similar keeps for the same call.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectScope {
  script: String,
}

impl ProjectScope {
  /// The scope *Approve for every run of this project* makes of `subject`: Orchester's `start_qa_app` with the QA
  /// script's git blob; `None` for any other request, and for a call that names no script.
  pub fn of(subject: &ApprovalSubject) -> Option<Self> {
    match subject {
      ApprovalSubject::McpTool {
        server,
        tool,
        reach: ToolReach::Script { blob },
      } if server == SERVER && tool == OrchesterTool::StartQaApp.as_str() && !blob.is_empty() => {
        Some(Self { script: blob.clone() })
      }
      _ => None,
    }
  }

  /// [`Self::of`] `approval`, read from the request alone: an MCP call's server, tool and target tell it, with no
  /// worktree to place a command in.
  pub fn of_request(approval: &PendingToolApproval) -> Option<Self> {
    if approval.command {
      return None;
    }
    let blob = QaApproval::script_of(approval.target.as_deref()?)?;
    Self::of(&ApprovalSubject::McpTool {
      server: approval.server.clone(),
      tool: approval.action.clone(),
      reach: ToolReach::Script { blob: blob.to_owned() },
    })
  }

  /// The scope a stored rule names; `None` for any but a `start_qa_app` rule kept to a script.
  pub fn stored(scope: RuleScope) -> Option<Self> {
    match scope {
      RuleScope::SimilarMcpTool {
        server,
        tool,
        folder: None,
        check: Some(script),
      } if server == SERVER && tool == OrchesterTool::StartQaApp.as_str() && !script.is_empty() => {
        Some(Self { script })
      }
      _ => None,
    }
  }

  /// How it is stored.
  pub fn rule_scope(&self) -> RuleScope {
    RuleScope::SimilarMcpTool {
      server: self.server().to_owned(),
      tool: self.tool().to_owned(),
      folder: None,
      check: Some(self.script.clone()),
    }
  }

  pub const fn server(&self) -> &'static str {
    SERVER
  }

  pub const fn tool(&self) -> &'static str {
    OrchesterTool::StartQaApp.as_str()
  }

  /// The QA script's git blob.
  pub fn script(&self) -> &str {
    &self.script
  }

  /// The script's blob abbreviated as git abbreviates one, for a person to read.
  pub fn short_script(&self) -> &str {
    self.script.get(..SHORT_SCRIPT).unwrap_or(&self.script)
  }
}

/// How many characters of a script's blob a person reads, as git abbreviates one.
const SHORT_SCRIPT: usize = 7;

/// A rule of the project ([`ProjectScope`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRule {
  pub id: ProjectRuleId,
  pub scope: ProjectScope,
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
}

/// The active project rule of `rules` that answers `subject`: one made of the same tool with the same script.
pub fn answering_in_project<'a>(rules: &'a [ProjectRule], subject: &ApprovalSubject) -> Option<&'a ProjectRule> {
  let scope = ProjectScope::of(subject)?;
  rules.iter().find(|rule| rule.is_active() && rule.scope == scope)
}
