//! Rules of the project (the person's decision of 2026-10-18, "Approval choices and waiting requests"): *Approve for
//! every run of this project* on a request of Orchester's `start_qa_app` approves it, and every later call of it in any
//! run of the project, while the worktree's QA script is the one approved, by its git blob. On a request of
//! `start_service` or `start_stack` (`docs/architecture/agent-testing.md`, "Service › approval") it keeps one rule per
//! service the start runs, keyed by the fingerprint of its program, arguments, variables and seed
//! ([`ServiceApproval`]): a later start is approved when every service it runs has a rule, and a change to any of them
//! asks again. A revoked rule asks again too. No other request is offered it: these tools run project programs outside
//! every sandbox, which the hard rules keep from auto mode's reviewer
//! ([`crate::run::review::ReviewHold::ProjectProgram`]), so without it a person approves them again in every run.
//! Orchester answers a request its rules cover itself in either approval mode, before auto mode looks at it. Only a
//! person makes such a rule ([`super::ApprovalAnswer::given_by`]); it lives with the project, not with the run that
//! made it, and a person can revoke it. A rule names a service's variables, never their values.

use serde::{Deserialize, Serialize};

use super::{
  rule::RuleScope,
  subject::{ApprovalSubject, ToolReach},
};
use crate::{
  id::RunId,
  orchester_tools::{OrchesterTool, QaApproval, SERVER},
  run::PendingToolApproval,
  service::{ServiceApproval, started_by},
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

/// What a rule of the project approves.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectScope {
  /// Orchester's `start_qa_app`, while the QA script is at one git blob. Stored as the [`RuleScope`] Approve similar
  /// keeps for the same call.
  QaApp { script: String },
  /// One service, in any start of `start_service` or `start_stack` that runs it, while its fingerprint is the one
  /// approved.
  Service(ServiceApproval),
}

impl ProjectScope {
  /// The scopes *Approve for every run of this project* makes of `subject`: Orchester's `start_qa_app` with the QA
  /// script's git blob, or each service a start of `start_service` or `start_stack` runs; none for any other request,
  /// and for a call that names no script or no service.
  pub fn of(subject: &ApprovalSubject) -> Vec<Self> {
    let ApprovalSubject::McpTool { server, tool, reach } = subject else {
      return Vec::new();
    };
    if server != SERVER {
      return Vec::new();
    }
    match (OrchesterTool::from_name(tool), reach) {
      (Some(OrchesterTool::StartQaApp), ToolReach::Script { blob }) if !blob.is_empty() => {
        vec![Self::QaApp { script: blob.clone() }]
      }
      (Some(OrchesterTool::StartService | OrchesterTool::StartStack), ToolReach::Services { services }) => {
        services.iter().cloned().map(Self::Service).collect()
      }
      _ => Vec::new(),
    }
  }

  /// [`Self::of`] `approval`, read from the request alone: an MCP call's server, tool and target tell it, with no
  /// worktree to place a command in.
  pub fn of_request(approval: &PendingToolApproval) -> Vec<Self> {
    let Some(target) = approval.target.as_deref().filter(|_| !approval.command) else {
      return Vec::new();
    };
    let reach = match OrchesterTool::from_name(&approval.action) {
      Some(OrchesterTool::StartQaApp) => {
        QaApproval::script_of(target).map(|blob| ToolReach::Script { blob: blob.to_owned() })
      }
      Some(OrchesterTool::StartService | OrchesterTool::StartStack) => {
        started_by(target).map(|services| ToolReach::Services { services })
      }
      _ => None,
    };
    reach.map_or_else(Vec::new, |reach| {
      Self::of(&ApprovalSubject::McpTool {
        server: approval.server.clone(),
        tool: approval.action.clone(),
        reach,
      })
    })
  }

  /// The `start_qa_app` scope a stored [`RuleScope`] names; `None` for any but a `start_qa_app` rule kept to a script.
  pub fn stored(scope: RuleScope) -> Option<Self> {
    match scope {
      RuleScope::SimilarMcpTool {
        server,
        tool,
        folder: None,
        check: Some(script),
      } if server == SERVER && tool == OrchesterTool::StartQaApp.as_str() && !script.is_empty() => {
        Some(Self::QaApp { script })
      }
      _ => None,
    }
  }

  /// How a `start_qa_app` scope is stored; `None` for a service's, which is stored as its [`ServiceApproval`].
  pub fn rule_scope(&self) -> Option<RuleScope> {
    match self {
      Self::QaApp { script } => Some(RuleScope::SimilarMcpTool {
        server: self.server().to_owned(),
        tool: self.tool().to_owned(),
        folder: None,
        check: Some(script.clone()),
      }),
      Self::Service(_) => None,
    }
  }

  pub const fn server(&self) -> &'static str {
    SERVER
  }

  /// The tool it approves; a service's rule approves `start_stack` too.
  pub const fn tool(&self) -> &'static str {
    match self {
      Self::QaApp { .. } => OrchesterTool::StartQaApp.as_str(),
      Self::Service(_) => OrchesterTool::StartService.as_str(),
    }
  }

  /// The QA script's git blob; `None` for a service's rule.
  pub fn script(&self) -> Option<&str> {
    match self {
      Self::QaApp { script } => Some(script),
      Self::Service(_) => None,
    }
  }

  /// The script's blob abbreviated as git abbreviates one, for a person to read; `None` for a service's rule.
  pub fn short_script(&self) -> Option<&str> {
    self.script().map(|script| script.get(..SHORT_SCRIPT).unwrap_or(script))
  }

  /// The service it approves; `None` for a `start_qa_app` rule.
  pub const fn service(&self) -> Option<&ServiceApproval> {
    match self {
      Self::QaApp { .. } => None,
      Self::Service(service) => Some(service),
    }
  }
}

/// How many characters of a script's blob a person reads, as git abbreviates one.
const SHORT_SCRIPT: usize = 7;

/// A rule of the project ([`ProjectScope`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRule {
  pub id: ProjectRuleId,
  pub scope: ProjectScope,
  /// The run whose request a person approved with it, and that request's id.
  pub made_in: RunId,
  pub made_by: String,
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

/// The active project rules of `rules` that answer `subject` together, one for each of its scopes
/// ([`ProjectScope::of`]): the same tool with the same script, or every service a start runs with the same
/// fingerprint. `None` when a scope has no active rule, and for a request no rule of the project answers.
pub fn answering_in_project<'a>(rules: &'a [ProjectRule], subject: &ApprovalSubject) -> Option<Vec<&'a ProjectRule>> {
  let scopes = ProjectScope::of(subject);
  if scopes.is_empty() {
    return None;
  }
  scopes
    .iter()
    .map(|scope| rules.iter().find(|rule| rule.is_active() && rule.scope == *scope))
    .collect()
}
