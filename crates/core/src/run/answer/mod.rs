//! The answers to a tool approval (`docs/design/orchester-workspace.md`, "Approval choices and waiting
//! requests", user decisions of 2026-10-02): Approve once, Approve similar in this run, Always allow,
//! Approve everything in this run, and Deny, which tells the agent and lets it continue. What each
//! answer covers is decided here; the runner applies it, the desktop shows it.
//!
//! Every answer still answers one request: Orchester never hands an agent's CLI a session grant. The
//! wider answers are kept by Orchester instead ([`ApprovalRule`], auto-approval), which answers each
//! later request itself, one at a time, and records who answered it ([`DecidedBy`]).

mod always;
mod auto;
mod rule;
mod subject;

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  always::{AlwaysAllowError, AlwaysUnavailable, allowing_command, allowing_tool, always_prefixes, listing},
  auto::{AutoAsk, AutoVerdict, auto_approves, auto_verdict},
  rule::{ApprovalRule, ApprovalRuleId, RuleScope, SimilarUnavailable, answering},
  subject::ApprovalSubject,
};
use super::PendingToolApproval;
use crate::permission::{CommandPrefix, category::CommandCategory};

/// A person's answer to one waiting request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalAnswer {
  /// This request only.
  Once,
  /// This request, and later ones of the run that are alike ([`RuleScope::similar`]).
  Similar,
  /// This request, and for good: a command's `prefix` goes into its profile's Allowed commands as a
  /// new profile version, an MCP tool into its server's Tools that run without asking.
  Always { prefix: Option<CommandPrefix> },
  /// This request, and every later one of the run that auto-approval approves ([`auto_verdict`]).
  ApproveRun,
  /// Not this request: the agent is told ([`denial_message`]) and continues.
  Deny,
}

impl ApprovalAnswer {
  pub const fn approves(&self) -> bool {
    !matches!(self, Self::Deny)
  }

  /// Who answered, when a person gives this answer.
  pub const fn decided_by(&self) -> DecidedBy {
    match self {
      Self::Once => DecidedBy::Person,
      Self::Similar => DecidedBy::PersonSimilar,
      Self::Always { .. } => DecidedBy::PersonAlways,
      Self::ApproveRun => DecidedBy::PersonRun,
      Self::Deny => DecidedBy::PersonDeny,
    }
  }
}

/// Who answered a request, as it is recorded and marked in the activity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecidedBy {
  /// A person approved it once.
  Person,
  /// A person approved it and made a rule of it (*· similar rule added*).
  PersonSimilar,
  /// A person approved it with Always allow.
  PersonAlways,
  /// A person approved it and everything later in the run that auto-approval approves.
  PersonRun,
  /// A person denied it; the agent was told and continued.
  PersonDeny,
  /// A rule of Approve similar answered it.
  SimilarRule,
  /// Always allow answered it before the agent's CLI knew the new rule.
  AlwaysAllowed,
  /// Auto-approval answered it.
  AutoApproved,
}

impl DecidedBy {
  pub const ALL: [Self; 8] = [
    Self::Person,
    Self::PersonSimilar,
    Self::PersonAlways,
    Self::PersonRun,
    Self::PersonDeny,
    Self::SimilarRule,
    Self::AlwaysAllowed,
    Self::AutoApproved,
  ];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Person => "person",
      Self::PersonSimilar => "person_similar",
      Self::PersonAlways => "person_always",
      Self::PersonRun => "person_run",
      Self::PersonDeny => "person_deny",
      Self::SimilarRule => "similar_rule",
      Self::AlwaysAllowed => "always_allowed",
      Self::AutoApproved => "auto_approved",
    }
  }

  pub const fn approves(self) -> bool {
    !matches!(self, Self::PersonDeny)
  }

  /// A person answered, rather than Orchester by a rule or auto-approval.
  pub const fn by_person(self) -> bool {
    matches!(
      self,
      Self::Person | Self::PersonSimilar | Self::PersonAlways | Self::PersonRun | Self::PersonDeny
    )
  }
}

/// A stored answer that names none of [`DecidedBy::ALL`]; the database was edited or damaged.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("`{0}` is not a known answer")]
pub struct UnknownAnswer(String);

impl FromStr for DecidedBy {
  type Err = UnknownAnswer;

  fn from_str(value: &str) -> Result<Self, UnknownAnswer> {
    Self::ALL
      .into_iter()
      .find(|answer| answer.as_str() == value)
      .ok_or_else(|| UnknownAnswer(value.to_owned()))
  }
}

/// How a request was answered, as the activity marks it (*approved by similar rule · pencil
/// batch_design*, *always allowed*, *auto-approved · Worktree*, *denied by you · the agent continued*).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDecision {
  pub decided_by: DecidedBy,
  /// A command's category, for its mark.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub category: Option<CommandCategory>,
  /// What the agent was told, for a denial.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub told: Option<String>,
}

/// What Codex App Server tells its model when Orchester declines a request (`decline`), word for word:
/// an MCP call ends with this error, a command with this failed `exec_command` output. Checked with
/// Codex 0.159.2 against a local mock of the Responses API; in both cases the turn went on.
pub const fn codex_denial(command: bool) -> &'static str {
  if command {
    r#"exec_command failed: CreateProcess { message: "Rejected("rejected by user")" }"#
  } else {
    "user rejected MCP tool call"
  }
}

/// What an agent is told when a person denies `approval`: the call does not run, and the agent goes on
/// without it. Claude Code hands it to the agent as the permission prompt's message; Codex declines
/// the request and tells the agent in its own words ([`codex_denial`]).
pub fn denial_message(approval: &PendingToolApproval) -> String {
  if approval.command {
    "Denied by the person reviewing this run. Continue without it.".to_owned()
  } else {
    format!(
      "The person reviewing this run denied {} {}. Continue without it.",
      approval.server, approval.action
    )
  }
}
