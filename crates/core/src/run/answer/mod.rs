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
mod note;
mod permitted;
mod project;
mod rule;
mod subject;

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  always::{AlwaysAllowError, AlwaysUnavailable, allowing_command, allowing_tool, always_prefixes, listing},
  auto::{AutoAsk, AutoVerdict, auto_approves, auto_verdict},
  note::{DenialNote, DenialNoteError},
  permitted::permitted,
  project::{ProjectRule, ProjectRuleId, ProjectScope, answering_in_project},
  rule::{ApprovalRule, ApprovalRuleId, RuleScope, SimilarUnavailable, answering},
  subject::{ApprovalSubject, ToolReach},
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
  /// This request, and later ones alike in every run of the project ([`ProjectScope`]): only Orchester's
  /// `start_qa_app`, while the QA script is unchanged.
  SimilarProject,
  /// This request, and for good: a command's `prefix` goes into its profile's Allowed commands as a
  /// new profile version, an MCP tool into its server's Tools that run without asking.
  Always { prefix: Option<CommandPrefix> },
  /// This request, and every later one of the run that auto-approval approves ([`auto_verdict`]).
  ApproveRun,
  /// Not this request: the agent is told ([`denial_message`]) and continues.
  Deny,
  /// Not this request, and the agent is told why in the denier's own words ([`denial_message`] with `message`), then
  /// continues. Who may deny may deny with a message.
  DenyWithMessage { message: DenialNote },
}

impl ApprovalAnswer {
  pub const fn approves(&self) -> bool {
    !matches!(self, Self::Deny | Self::DenyWithMessage { .. })
  }

  /// The words a denial with a message tells the agent; `None` for any other answer.
  pub const fn denial_note(&self) -> Option<&DenialNote> {
    match self {
      Self::DenyWithMessage { message } => Some(message),
      _ => None,
    }
  }

  /// Who answered, when a person gives this answer.
  pub const fn decided_by(&self) -> DecidedBy {
    match self {
      Self::Once => DecidedBy::Person,
      Self::Similar | Self::SimilarProject => DecidedBy::PersonSimilar,
      Self::Always { .. } => DecidedBy::PersonAlways,
      Self::ApproveRun => DecidedBy::PersonRun,
      Self::Deny | Self::DenyWithMessage { .. } => DecidedBy::PersonDeny,
    }
  }

  /// Who answered, when `answerer` gives this answer to a request about `subject`. Automation answers
  /// one request at a time: it denies any, with a message or without, and approves once only what auto-approval
  /// would approve ([`auto_verdict`]); what auto-approval leaves to a person, and the wider answers, which change
  /// later requests, stay with a person.
  pub fn given_by(&self, answerer: Answerer, subject: &ApprovalSubject) -> Result<DecidedBy, AutomationAnswerError> {
    match (answerer, self) {
      (Answerer::Person, answer) => Ok(answer.decided_by()),
      (Answerer::Automation, Self::Deny | Self::DenyWithMessage { .. }) => Ok(DecidedBy::AutomationDeny),
      (Answerer::Automation, Self::Once) => match auto_verdict(subject) {
        AutoVerdict::Approve(_) => Ok(DecidedBy::Automation),
        AutoVerdict::Ask(ask) => Err(AutomationAnswerError::PersonOnly(ask)),
      },
      (Answerer::Automation, Self::Similar | Self::SimilarProject | Self::Always { .. } | Self::ApproveRun) => {
        Err(AutomationAnswerError::Wider)
      }
    }
  }
}

/// Who gives an answer to a waiting request, or decides a waiting plan: the person in the app, or
/// automation through Orchester's own MCP server, which a person allows in the app's settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answerer {
  #[default]
  Person,
  Automation,
}

/// An answer automation may not give.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AutomationAnswerError {
  #[error("automation only approves a request once or denies it")]
  Wider,
  /// Auto-approval leaves this request to a person, for this reason; only a person approves it.
  #[error("only a person can approve this request: {0:?}")]
  PersonOnly(AutoAsk),
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
  /// The step's own permissions run every part of the command on its own ([`permitted`]).
  Permissions,
  /// Automation approved it once, through Orchester's MCP server.
  Automation,
  /// Automation denied it, through Orchester's MCP server; the agent was told and continued.
  AutomationDeny,
  /// Auto mode's reviewer approved it ([`super::review`]).
  Reviewer,
}

impl DecidedBy {
  pub const ALL: [Self; 12] = [
    Self::Person,
    Self::PersonSimilar,
    Self::PersonAlways,
    Self::PersonRun,
    Self::PersonDeny,
    Self::SimilarRule,
    Self::AlwaysAllowed,
    Self::AutoApproved,
    Self::Permissions,
    Self::Automation,
    Self::AutomationDeny,
    Self::Reviewer,
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
      Self::Permissions => "permissions",
      Self::Automation => "automation",
      Self::AutomationDeny => "automation_deny",
      Self::Reviewer => "reviewer",
    }
  }

  pub const fn approves(self) -> bool {
    !matches!(self, Self::PersonDeny | Self::AutomationDeny)
  }

  /// A person answered, rather than Orchester by a rule or auto-approval.
  pub const fn by_person(self) -> bool {
    matches!(
      self,
      Self::Person | Self::PersonSimilar | Self::PersonAlways | Self::PersonRun | Self::PersonDeny
    )
  }

  /// Automation answered, through Orchester's MCP server.
  pub const fn by_automation(self) -> bool {
    matches!(self, Self::Automation | Self::AutomationDeny)
  }

  /// Someone answered a request that waited for them, a person or automation, rather than Orchester
  /// itself as the request came.
  pub const fn answered_waiting(self) -> bool {
    self.by_person() || self.by_automation() || matches!(self, Self::Reviewer)
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
  /// The command read a credential location, which its mark names instead of the category.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub secrets: bool,
  /// What the agent was told, for a denial.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub told: Option<String>,
  /// How long the request waited for a person, in milliseconds; `None` when Orchester answered it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub waited_ms: Option<u64>,
  /// What auto mode did with the request before it was answered: its approval, or why it asked.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub review: Option<super::review::Review>,
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

/// What an agent is told when `by`, a person or automation, denies `approval`: the call does not run,
/// and the agent goes on without it. Automation's denial names no person, as none decided. Claude Code
/// hands it to the agent as the permission prompt's message; Codex declines the request and tells the
/// agent in its own words ([`codex_denial`]).
pub fn denial_message(approval: &PendingToolApproval, by: Answerer) -> String {
  match (by, approval.command) {
    (Answerer::Person, true) => "Denied by the person reviewing this run. Continue without it.".to_owned(),
    (Answerer::Person, false) => format!(
      "The person reviewing this run denied {} {}. Continue without it.",
      approval.server, approval.action
    ),
    (Answerer::Automation, true) => {
      "Denied by automation through Orchester's MCP server. Continue without it.".to_owned()
    }
    (Answerer::Automation, false) => format!(
      "Automation denied {} {} through Orchester's MCP server. Continue without it.",
      approval.server, approval.action
    ),
  }
}

/// What an agent is told when `by` denies `approval` with `message` (*Deny with a message*): who denied it, then
/// their words, which take the place of *Continue without it*. Claude Code and Orchester's tools hand it to the agent
/// as the denial's reason; Codex, whose decline carries no words, gets it as the next message of its turn.
pub fn denial_with_message(approval: &PendingToolApproval, by: Answerer, message: &DenialNote) -> String {
  match (by, approval.command) {
    (Answerer::Person, true) => format!("Denied by the person reviewing this run: {message}"),
    (Answerer::Person, false) => format!(
      "The person reviewing this run denied {} {}: {message}",
      approval.server, approval.action
    ),
    (Answerer::Automation, true) => format!("Denied by automation through Orchester's MCP server: {message}"),
    (Answerer::Automation, false) => format!(
      "Automation denied {} {} through Orchester's MCP server: {message}",
      approval.server, approval.action
    ),
  }
}
