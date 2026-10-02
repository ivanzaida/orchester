//! Which native commands run, ask a person, or are refused (`docs/architecture/native-agent.md` §5.2,
//! decision 1): a shell command asks; with *Allowed commands* a listed command runs, a known-safe
//! read-only one runs once the list has an entry, and every other command asks; with *Ask before running
//! commands* every command asks. Refused are only calls that can never run: a command that cannot be
//! read, or a working folder outside the worktree.

use thiserror::Error;

use super::{CommandForm, CommandRefusal, CommandSyntax, ParsedCommand};
use crate::{
  native::{scope::FileScope, tools::RunCommand},
  permission::{AllowedCommands, CommandPermission, known_safe},
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandPolicyError {
  /// *Any command in the worktree* needs an OS sandbox the native agent does not have yet (§5.4, P5).
  #[error("a native agent cannot confine commands to the worktree")]
  AnyCommandUnavailable,
}

/// What happens to a `run_command` call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandVerdict {
  Run(ParsedCommand),
  /// A person decides.
  Ask(ParsedCommand),
  Refuse(CommandRefusal),
}

/// The command permission of a native step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPolicy {
  /// `None`: ask before every command.
  allowed: Option<AllowedCommands>,
}

impl CommandPolicy {
  pub fn new(permission: &CommandPermission) -> Result<Self, CommandPolicyError> {
    match permission {
      CommandPermission::AskBefore => Ok(Self { allowed: None }),
      CommandPermission::Allowed(list) => Ok(Self {
        allowed: Some(list.clone()),
      }),
      CommandPermission::AnyInWorktree => Err(CommandPolicyError::AnyCommandUnavailable),
    }
  }

  /// The verdict on `call` in `scope`, its command split as on this platform.
  pub fn classify(&self, call: &RunCommand, scope: &FileScope) -> CommandVerdict {
    let command = match ParsedCommand::parse(call, CommandSyntax::HOST, scope) {
      Ok(command) => command,
      Err(refusal) => return CommandVerdict::Refuse(refusal),
    };
    if self.runs(&command, scope) {
      CommandVerdict::Run(command)
    } else {
      CommandVerdict::Ask(command)
    }
  }

  fn runs(&self, command: &ParsedCommand, scope: &FileScope) -> bool {
    let (Some(list), CommandForm::Direct { argv }) = (&self.allowed, command.form()) else {
      return false;
    };
    if list.allows_argv(argv) {
      return true;
    }
    !list.is_empty()
      && known_safe::is_known_safe(argv)
      && known_safe::path_candidates(argv)
        .into_iter()
        .all(|arg| scope.argument_readable(command.workdir(), arg))
  }
}
