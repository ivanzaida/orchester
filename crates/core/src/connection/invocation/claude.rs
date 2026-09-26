//! How an agent step starts the Claude Code CLI (`.tmp/cli-probe/REPORT.md`, "Recommended argv"):
//!
//! `claude -p --output-format stream-json --verbose --model <model> [--effort <e>]
//!  --strict-mcp-config --setting-sources "" --disable-slash-commands --tools <tools>
//!  <permissions> (--session-id <new uuid> | --resume <session>)`
//!
//! in the worktree as the process's working directory, with the prompt on standard input.
//!
//! - `stream-json` needs `--verbose` in print mode; without it the CLI exits with an error.
//! - Isolation from the user's own setup: `--strict-mcp-config` loads no MCP server, `--disable-slash-commands` no
//!   skills, and `--setting-sources ""` no settings file at all. The user's settings carry allow rules that would widen
//!   the permissions below; a repository's `.claude/settings.json` could do the same, or run hooks, and in a writing
//!   step the agent could edit that file for the invocations after it.
//! - `--tools` offers only the built-in tools a step needs, so scheduling, remote triggers, worktree switching and
//!   sub-agents are not available at all.
//! - Permissions, from the step's repository access. Read-only runs in `dontAsk` with the tools that change files
//!   denied, and without `Bash`, which could change them too. Writing runs in `acceptEdits` (file edits are accepted,
//!   in print mode nothing else is ever asked) and allows only git commands; any other command is denied and the CLI
//!   reports the denial. Bypass modes are never used. Claude Code's own sandbox is not used: it is not supported on
//!   Windows. The agent profile permissions (repository access, allowed commands, network) will replace this mapping.
//! - Sessions: a new session gets an id Orchester chose (`--session-id`); a continue after a pause or a limit resumes
//!   the session the CLI reported (`--resume`). Sessions are kept on disk (no `--no-session-persistence`) so that they
//!   can be resumed.
//! - No `--json-schema`: its result shape could not be verified (the probe's CLI was not signed in). The prompt asks
//!   for the step result as the final message, and the CLI's `is_error` decides when it is not one.

use std::{ffi::OsString, path::Path};

use uuid::Uuid;

use super::{InvocationError, agent_command};
use crate::{command::Command, resource::RepositoryAccess};

/// Tools a read-only step can use.
const READ_TOOLS: &str = "Read,Glob,Grep,TodoWrite";
/// Tools a writing step can use.
const WRITE_TOOLS: &str = "Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash";
/// Denied to a read-only step, even if a tool list offered them.
const READ_ONLY_DENIED: [&str; 6] = ["Edit", "Write", "NotebookEdit", "WebFetch", "WebSearch", "Bash"];
/// Commands a writing step may run: git only, as prefix rules.
const WRITE_ALLOWED: [&str; 1] = ["Bash(git:*)"];

/// Which session an invocation works in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaudeSession<'a> {
  /// A new session with this id.
  New(Uuid),
  /// The session the CLI reported earlier, continued.
  Resume(&'a str),
}

impl<'a> ClaudeSession<'a> {
  /// Continues `resume` when given, else starts a new session with a fresh id.
  pub fn continuing(resume: Option<&'a str>) -> Self {
    resume.map_or_else(|| Self::New(Uuid::now_v7()), Self::Resume)
  }
}

/// Everything that decides how one Claude Code invocation starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaudeInvocation<'a> {
  pub executable: &'a Path,
  pub model: &'a str,
  /// The profile's effort; `None` leaves the CLI's default.
  pub effort: Option<&'a str>,
  pub access: RepositoryAccess,
  pub worktree: &'a Path,
  pub session: ClaudeSession<'a>,
}

impl ClaudeInvocation<'_> {
  /// Arguments after the program, in order. They do not differ between Windows and macOS.
  pub fn args(&self) -> Vec<OsString> {
    let mut args: Vec<String> = [
      "-p",
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
        "--tools",
      ]
      .map(str::to_owned),
    );
    match self.access {
      RepositoryAccess::ReadOnly => {
        args.extend([READ_TOOLS, "--permission-mode", "dontAsk", "--disallowedTools"].map(str::to_owned));
        args.extend(READ_ONLY_DENIED.map(str::to_owned));
      }
      RepositoryAccess::Write => {
        args.extend([WRITE_TOOLS, "--permission-mode", "acceptEdits", "--allowedTools"].map(str::to_owned));
        args.extend(WRITE_ALLOWED.map(str::to_owned));
      }
    }
    match self.session {
      ClaudeSession::New(id) => args.extend(["--session-id".to_owned(), id.to_string()]),
      ClaudeSession::Resume(session) => args.extend(["--resume".to_owned(), session.to_owned()]),
    }
    args.into_iter().map(OsString::from).collect()
  }

  /// The command that runs the step: in the worktree, with `prompt` on standard input and only
  /// the allowed part of `environment`.
  pub fn command(
    &self,
    prompt: String,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
  ) -> Result<Command, InvocationError> {
    agent_command(self.executable, self.args(), self.worktree, prompt, environment)
  }
}
