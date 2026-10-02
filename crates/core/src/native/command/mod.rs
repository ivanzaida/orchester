//! The native agent's `run_command` (`docs/architecture/native-agent.md` §4.6, §5.2): a call read into
//! the command Orchester would start, and the policy that decides whether it runs, asks a person or is
//! refused.

mod policy;
mod syntax;

use std::path::{Path, PathBuf};

use thiserror::Error;

pub use self::{
  policy::{CommandPolicy, CommandPolicyError, CommandVerdict},
  syntax::{CommandSyntax, SyntaxError},
};
use super::{
  scope::{FileScope, ScopeError},
  tools::RunCommand,
};
use crate::permission::known_safe;

/// Global options every git call gets: an fsmonitor setting could start a program on `git status`, and
/// a pager would wait for input that never comes.
const GIT_GLOBAL: [&str; 3] = ["-c", "core.fsmonitor=false", "--no-pager"];
/// Git subcommands that print diffs, and the options that keep configured diff programs from running.
const GIT_DIFFS: [&str; 3] = ["diff", "log", "show"];
const GIT_DIFF_OPTIONS: [&str; 2] = ["--no-ext-diff", "--no-textconv"];
/// `git blame` runs a configured textconv program by default; it takes no diff program.
const GIT_BLAME_OPTIONS: [&str; 1] = ["--no-textconv"];

/// Why a command can never run: the agent gets this as an error result.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommandRefusal {
  #[error(transparent)]
  Syntax(#[from] SyntaxError),
  #[error("the working folder must be inside the worktree: {0}")]
  Workdir(ScopeError),
}

/// How a command is started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandForm {
  /// A program found on the agent's `PATH`, started without a shell, with these arguments (program
  /// first) as the model wrote them.
  Direct { argv: Vec<String> },
  /// The string, run by the platform shell.
  Shell,
}

/// A `run_command` call as Orchester would start it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCommand {
  text: String,
  form: CommandForm,
  workdir: PathBuf,
  timeout_seconds: u32,
}

impl ParsedCommand {
  /// The command of `call`, split by `syntax`, with its working folder resolved in `scope`.
  pub fn parse(call: &RunCommand, syntax: CommandSyntax, scope: &FileScope) -> Result<Self, CommandRefusal> {
    let form = if call.shell() {
      CommandForm::Shell
    } else {
      CommandForm::Direct {
        argv: syntax.split(call.command())?,
      }
    };
    let workdir = scope.workdir(call.workdir()).map_err(CommandRefusal::Workdir)?;
    Ok(Self {
      text: call.command().to_owned(),
      form,
      workdir,
      timeout_seconds: call.timeout_seconds(),
    })
  }

  /// The command as the model wrote it, as a person is shown it.
  pub fn text(&self) -> &str {
    &self.text
  }

  pub const fn form(&self) -> &CommandForm {
    &self.form
  }

  pub fn workdir(&self) -> &Path {
    &self.workdir
  }

  pub const fn timeout_seconds(&self) -> u32 {
    self.timeout_seconds
  }

  /// The program and arguments to start, `None` for a shell command. Git gets [`GIT_GLOBAL`] before its
  /// subcommand, its diff subcommands also [`GIT_DIFF_OPTIONS`] after it, and `blame`
  /// [`GIT_BLAME_OPTIONS`]. The program's name is matched as the platform finds it, so `Git` on macOS is
  /// hardened too.
  pub fn program_args(&self) -> Option<Vec<String>> {
    let CommandForm::Direct { argv } = &self.form else {
      return None;
    };
    let is_git = argv
      .first()
      .and_then(|program| known_safe::program_name(program))
      .as_deref()
      == Some("git");
    if !is_git {
      return Some(argv.clone());
    }
    let mut args = vec![argv[0].clone()];
    args.extend(GIT_GLOBAL.map(str::to_owned));
    if let Some(subcommand) = argv.get(1) {
      args.push(subcommand.clone());
      if GIT_DIFFS.contains(&subcommand.as_str()) {
        args.extend(GIT_DIFF_OPTIONS.map(str::to_owned));
      } else if subcommand == "blame" {
        args.extend(GIT_BLAME_OPTIONS.map(str::to_owned));
      }
    }
    args.extend(argv.iter().skip(2).cloned());
    Some(args)
  }
}
