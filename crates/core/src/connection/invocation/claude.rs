//! How an agent step starts the Claude Code CLI (`.tmp/cli-probe/REPORT.md`, "Recommended argv"):
//!
//! `claude -p --output-format stream-json --verbose --model <model> [--effort <e>]
//!  --strict-mcp-config --setting-sources "" --disable-slash-commands <permissions>
//!  (--session-id <new uuid> | --resume <session>)`
//!
//! in the worktree as the process's working directory, with the prompt on standard input.
//!
//! - `stream-json` needs `--verbose` in print mode; without it the CLI exits with an error.
//! - Isolation from the user's own setup: `--strict-mcp-config` loads no MCP server, `--disable-slash-commands` no
//!   skills, and `--setting-sources ""` no settings file at all. The user's settings carry allow rules that would widen
//!   the permissions; a repository's `.claude/settings.json` could do the same, or run hooks, and in a writing step the
//!   agent could edit that file for the invocations after it.
//! - Permissions: the step's enforced permissions as [`crate::permission::claude::permission_args`] maps them (tools
//!   offered, permission mode, denied tools, allowed command prefixes). Bypass modes are never used.
//! - Sessions: a new session gets an id Orchester chose (`--session-id`); a continue after a pause or a limit resumes
//!   the session the CLI reported (`--resume`), with the permissions given again. Sessions are kept on disk (no
//!   `--no-session-persistence`) so that they can be resumed.
//! - No `--json-schema`: its result shape could not be verified (the probe's CLI was not signed in). The prompt asks
//!   for the step result as the final message, and the CLI's `is_error` decides when it is not one.

use std::{ffi::OsString, path::Path};

use uuid::Uuid;

use super::{InvocationError, agent_command};
use crate::{
  command::Command,
  permission::{capability::EnforcedPermissions, claude::permission_args},
  resource::CommandEnvironment,
};

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
  /// The step's effective permissions, as Claude Code enforces them.
  pub permissions: &'a EnforcedPermissions,
  pub worktree: &'a Path,
  pub session: ClaudeSession<'a>,
}

impl ClaudeInvocation<'_> {
  /// Arguments after the program, in order. They do not differ between Windows and macOS: the
  /// platform only decides which permissions are enforceable.
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
      ]
      .map(str::to_owned),
    );
    args.extend(permission_args(self.permissions));
    match self.session {
      ClaudeSession::New(id) => args.extend(["--session-id".to_owned(), id.to_string()]),
      ClaudeSession::Resume(session) => args.extend(["--resume".to_owned(), session.to_owned()]),
    }
    args.into_iter().map(OsString::from).collect()
  }

  /// The command that runs the step: in the worktree, with `prompt` on standard input, the
  /// allowed part of `environment` and the variables the project adds (`project`).
  pub fn command(
    &self,
    prompt: String,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    agent_command(
      self.executable,
      self.args(),
      self.worktree,
      prompt,
      (environment, project),
    )
  }
}
