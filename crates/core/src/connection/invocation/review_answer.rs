//! How the read-only turn that answers a person's review question (`crate::review`) starts Claude Code, as the run's
//! implementing step's agent, in the run's worktree:
//!
//! `claude -p --input-format stream-json --output-format stream-json --verbose --model <model> [--effort <e>]
//!  --strict-mcp-config --setting-sources "" --disable-slash-commands
//!  --settings {"permissions":{"blockReadsOutsideWorkingDirectories":true}} --tools Read,Glob,Grep
//!  --disallowedTools Read(//<denied folder>/**)… Read(//<denied file>)… --permission-mode default --max-turns 30
//!  (--session-id <new uuid> | --resume <step's session> --fork-session)`
//!
//! with the question as the one stream-json user message on standard input, which the runner closes once the CLI
//! ended its turn. Read-only as the Orchestrator chat's turn is ([`super::ClaudeChat`]): only `Read`, `Glob` and `Grep`
//! are offered, so it has no shell, no edit tool and no web tool, reads stay in the worktree, and the places that hold
//! secrets are denied by rule. No MCP server is given: `--strict-mcp-config` without a configuration leaves it none.
//! The step's own session is resumed as a fork, so the agent answers with what it did in mind while the step's session
//! stays as the step left it; the thread's earlier messages are in the question's prompt.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use super::{
  ClaudeSession, InvocationError, agent_environment,
  chat::{BLOCK_OUTSIDE_READS, READ_TOOLS},
  first_line, without_auto_memory,
};
use crate::{
  command::Command,
  orchestrator::MAX_TURNS,
  permission::claude::{read_file_rule, read_rule},
  resource::CommandEnvironment,
};

/// Everything that decides how the turn that answers a review question starts Claude Code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaudeReviewAnswer<'a> {
  pub executable: &'a Path,
  pub model: &'a str,
  /// `None` leaves the CLI's default.
  pub effort: Option<&'a str>,
  /// The run's worktree, the working directory.
  pub worktree: &'a Path,
  /// The implementing step's session, resumed as a fork, or a new one when it has none.
  pub session: ClaudeSession<'a>,
  /// Folders the agent must not read even where the CLI would ask: they hold secrets.
  pub denied: &'a [PathBuf],
  /// Files the agent must not read either.
  pub denied_files: &'a [PathBuf],
}

impl ClaudeReviewAnswer<'_> {
  /// Arguments after the program, in order; the same on Windows and macOS.
  pub fn args(&self) -> Vec<String> {
    let mut args: Vec<String> = [
      "-p",
      "--input-format",
      "stream-json",
      "--output-format",
      "stream-json",
      "--verbose",
      "--model",
      self.model,
    ]
    .map(str::to_owned)
    .to_vec();
    if let Some(effort) = self.effort {
      args.extend(["--effort".to_owned(), effort.to_owned()]);
    }
    args.extend(
      [
        "--strict-mcp-config",
        "--setting-sources",
        "",
        "--disable-slash-commands",
        "--settings",
        BLOCK_OUTSIDE_READS,
      ]
      .map(str::to_owned),
    );
    args.extend(["--tools".to_owned(), READ_TOOLS.join(",")]);
    let denied: Vec<String> = self
      .denied
      .iter()
      .filter_map(|folder| read_rule(folder))
      .chain(self.denied_files.iter().filter_map(|file| read_file_rule(file)))
      .collect();
    if !denied.is_empty() {
      args.push("--disallowedTools".to_owned());
      args.extend(denied);
    }
    args.extend(["--permission-mode".to_owned(), "default".to_owned()]);
    args.extend(["--max-turns".to_owned(), MAX_TURNS.to_string()]);
    match self.session {
      ClaudeSession::New(id) => args.extend(["--session-id".to_owned(), id.to_string()]),
      ClaudeSession::Resume(session) => {
        args.extend(["--resume".to_owned(), session.to_owned(), "--fork-session".to_owned()]);
      }
    }
    args
  }

  /// The command of the turn that answers `question`: in the worktree, with the allowed part of `environment` and the
  /// variables the project adds (`project`).
  pub fn command(
    &self,
    question: &str,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    if self.worktree.to_str().is_none() {
      return Err(InvocationError::NotUnicode(self.worktree.to_path_buf()));
    }
    let mut variables = agent_environment(environment);
    project.add_to(&mut variables);
    let command = Command::new(self.executable, self.args())
      .with_current_dir(self.worktree)
      .with_environment(variables)
      .with_interactive_input(first_line(question, &[])?);
    Ok(without_auto_memory(command))
  }
}
