//! Rules a run keeps from its wider answers (design `k4cz1C`, "Approval choices and waiting requests"):
//! Approve similar makes one for the same program and subcommand in the same category, reaching outside
//! the worktree only where the approved command did, or for one tool of one MCP server (for Orchester's tools that
//! read outside the worktree: reads and searches in the folder the approved call read in and below); no rule ever
//! covers a command part that rewrites history, deletes outside the worktree, reads a credential or was
//! not read to the end; Always allow of a command makes one for its prefix, so the run keeps allowing it
//! although it started with the profile version before. Rules live with the run, end with it, never
//! reach the agent's CLI, and a similar rule can be revoked; Orchester answers each later request a
//! rule covers itself.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::subject::ApprovalSubject;
use crate::{
  id::RunId,
  permission::{
    CommandPrefix,
    category::{ClassifiedCommand, CommandCategory, CommandPart, SimilarCommand},
  },
  time::Timestamp,
};

/// A rule's number in the project, as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApprovalRuleId(i64);

impl ApprovalRuleId {
  pub const fn new(value: i64) -> Self {
    Self(value)
  }

  pub const fn get(self) -> i64 {
    self.0
  }
}

/// What a rule answers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleScope {
  /// Commands whose asking part has this program and subcommand in this category.
  SimilarCommand { command: SimilarCommand },
  /// Any call of `tool` of MCP server `server`, whatever its arguments. With a `folder` (Orchester's tools that read
  /// outside the worktree), any call of those tools that reads in it or below it. Rules made before folders were kept
  /// have none.
  SimilarMcpTool {
    server: String,
    tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    folder: Option<String>,
  },
  /// Commands this prefix allows, added to the profile's Allowed commands by Always allow.
  AlwaysCommand { prefix: CommandPrefix },
}

/// Why Approve similar cannot answer a request (`ZJt4d`, `k4cz1C`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimilarUnavailable {
  /// Part of the command is not read to the end, or its arguments are a command for another machine
  /// (`ssh host …`): Orchester cannot tell which later commands are alike.
  Unknown,
  /// Several parts of the command would ask by themselves, not all alike.
  SeveralAsk,
  /// It rewrites or discards history, or deletes outside the worktree: a person answers each.
  Destructive,
  /// It reads a credential location or store: a person answers each.
  Secrets,
}

impl RuleScope {
  /// The rule Approve similar makes of `subject`: the command's asking part (the strictest), or the
  /// call's server and tool.
  pub fn similar(subject: &ApprovalSubject) -> Result<Self, SimilarUnavailable> {
    match subject {
      ApprovalSubject::McpTool { server, tool, folder } => Ok(Self::SimilarMcpTool {
        server: server.clone(),
        tool: tool.clone(),
        folder: folder.clone(),
      }),
      ApprovalSubject::Command(command) => similar_command(command).map(|command| Self::SimilarCommand { command }),
    }
  }

  /// A rule of Approve similar, which a person can revoke; Always allow's lives in the profile.
  pub const fn revocable(&self) -> bool {
    !matches!(self, Self::AlwaysCommand { .. })
  }

  fn covers_part(&self, part: &CommandPart) -> bool {
    match self {
      Self::SimilarCommand { command } => part.is_similar(command),
      Self::AlwaysCommand { prefix } => !part.always_asks() && prefix.covers_argv(part.words()),
      Self::SimilarMcpTool { .. } => false,
    }
  }
}

fn similar_command(command: &ClassifiedCommand) -> Result<SimilarCommand, SimilarUnavailable> {
  if command
    .parts()
    .iter()
    .any(|part| part.category() == CommandCategory::Unknown)
  {
    return Err(SimilarUnavailable::Unknown);
  }
  if command.reads_secrets() {
    return Err(SimilarUnavailable::Secrets);
  }
  if command.category() == CommandCategory::HistoryDestructive {
    return Err(SimilarUnavailable::Destructive);
  }
  let asking: Vec<&CommandPart> = command.asking().collect();
  if let [first, rest @ ..] = asking.as_slice()
    && !rest.is_empty()
  {
    // Several asking parts alike (`sed -i …; sed -i …` in the worktree) make one rule that covers each.
    let rule = first.similar().ok_or(SimilarUnavailable::Unknown)?;
    return if rest.iter().all(|part| part.similar().as_ref() == Some(&rule)) {
      Ok(rule)
    } else {
      Err(SimilarUnavailable::SeveralAsk)
    };
  }
  command
    .forcing()
    .and_then(CommandPart::similar)
    .ok_or(SimilarUnavailable::Unknown)
}

/// A rule of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalRule {
  pub id: ApprovalRuleId,
  pub run: RunId,
  pub scope: RuleScope,
  pub created_at: Timestamp,
  /// When a person revoked it; it answers nothing since.
  pub revoked_at: Option<Timestamp>,
  /// Requests it answered.
  pub answered: u32,
}

impl ApprovalRule {
  pub const fn is_active(&self) -> bool {
    self.revoked_at.is_none()
  }
}

/// The active rule of `rules` that answers `subject`, if one does. A command is answered when a rule
/// covers one of its parts and each other part is covered by a rule too or is a plain read (Read-only,
/// reading no credential and installing nothing); a part Orchester cannot read is never covered. An MCP
/// call is answered by the rule for its server and tool, or by a rule of its server for the folder it reads in.
pub fn answering<'a>(rules: &'a [ApprovalRule], subject: &ApprovalSubject) -> Option<&'a ApprovalRule> {
  let mut active = rules.iter().filter(|rule| rule.is_active());
  match subject {
    ApprovalSubject::McpTool { server, tool, folder } => active.find(|rule| match &rule.scope {
      RuleScope::SimilarMcpTool {
        server: s,
        tool: t,
        folder: None,
      } => s == server && t == tool,
      // Only Orchester's calls outside the worktree name a folder: such a rule covers each of them that reads there.
      RuleScope::SimilarMcpTool {
        server: s,
        folder: Some(within),
        ..
      } => {
        s == server
          && folder
            .as_deref()
            .is_some_and(|folder| Path::new(folder).starts_with(within))
      }
      _ => false,
    }),
    ApprovalSubject::Command(command) => {
      let rules: Vec<&ApprovalRule> = active.collect();
      let covering = |part: &CommandPart| rules.iter().copied().find(|rule| rule.scope.covers_part(part));
      let plain =
        |part: &CommandPart| part.category() == CommandCategory::ReadOnly && !part.reads_secrets() && !part.installs();
      if command
        .parts()
        .iter()
        .any(|part| covering(part).is_none() && !plain(part))
      {
        return None;
      }
      // The rule of the part that names the command, else of any part.
      command
        .forcing()
        .and_then(covering)
        .or_else(|| command.parts().iter().find_map(covering))
    }
  }
}
