//! How an agent step starts the Claude Code CLI (`.tmp/cli-probe/REPORT.md`, "Recommended argv"):
//!
//! `claude -p --output-format stream-json --verbose --model <model> [--effort <e>]
//!  --strict-mcp-config --setting-sources "" --disable-slash-commands [--mcp-config <file> [<approvals>]]
//!  [--permission-prompt-tool mcp__Orchester__approve] <permissions> [--add-dir <folder>]... [--add-dir <skills>]
//!  (--session-id <new uuid> | --resume <session>)`
//!
//! in the worktree as the process's working directory, with the prompt on standard input.
//!
//! - `stream-json` needs `--verbose` in print mode; without it the CLI exits with an error.
//! - Isolation from the user's own setup: `--strict-mcp-config` loads no MCP server but the invocation's own,
//!   `--disable-slash-commands` no skills, and `--setting-sources ""` no settings file at all. The user's settings
//!   carry allow rules that would widen the permissions; a repository's `.claude/settings.json` could do the same, or
//!   run hooks, and in a writing step the agent could edit that file for the invocations after it.
//! - MCP servers: only those the profile chooses, from a file the runner writes for the invocation
//!   ([`super::ClaudeMcpConfig`]); their tools are allowed with `--allowedTools mcp__<server>` rules.
//! - Approvals: a step that asks a person before commands or MCP actions also gets the configuration file of
//!   Orchester's permission prompt relay and `--permission-prompt-tool` naming its tool
//!   ([`crate::connection::claude_approval`]); on a resumed session too, as the prompt is not part of the session.
//! - Permissions: the step's enforced permissions as [`crate::permission::claude::permission_args`] maps them (tools
//!   offered, permission mode, denied tools, allowed command prefixes, and the edit rules of the worktree and the extra
//!   writable folders when the step does not run in `acceptEdits`). Bypass modes are never used.
//! - Extra writable folders ([`crate::permission::writable`]), for an agent that edits files: one `--add-dir` each, so
//!   its edit tools may write there as in the working directory (in `acceptEdits`; with edit rules, by the folder's
//!   `Edit` allow rule). Its commands need nothing: Claude Code does not confine them to any folder. Checked 2026-09-28
//!   with Claude Code 2.1.89 without a model turn: the debug log shows each existing folder added as a permission
//!   directory (`cliArg`) and a missing one left out; the folder's own `.claude/settings.json` is looked up but, with
//!   `--setting-sources ""`, its rules were not applied.
//! - Repository agent files ([`crate::agent_files`]): `--setting-sources ""` already keeps `CLAUDE.md` and
//!   `.claude/skills` out (checked 2026-09-29 against the request the CLI sends), so the project's setting adds
//!   nothing.
//! - Orchester's skills ([`crate::resource::Skill`]) are placed in a folder outside the worktree, which `--add-dir`
//!   lets the agent read (Claude Code asks before reading outside its working directories, and print mode denies what
//!   it would ask). In `acceptEdits` an agent that edits files may also write there, as in every working directory; the
//!   folder is its own and written again when it starts. A step that edits by rule
//!   ([`crate::permission::capability::Mechanism::ClaudeEditRules`]) gets no rule for it, so it only reads it. The code
//!   index of the run's base revision (`crate::code_index`) is given the same way.
//! - Sessions: a new session gets an id Orchester chose (`--session-id`); a continue after a pause or a limit resumes
//!   the session the CLI reported (`--resume`), with the permissions given again. Sessions are kept on disk (no
//!   `--no-session-persistence`) so that they can be resumed.
//! - The step result: `--json-schema` with [`STEP_RESULT_SCHEMA`] on every session, new or resumed (the prompt asks for
//!   the result in both). The CLI then reports the result as `structured_output`; 2.1.284 verified with Opus 5.5
//!   (`docs/evidence/claude-code-e2e-2026-09-30.md`). The schema is one argument of JSON: a native `claude.exe` and
//!   npm's Node.js launcher (started through Node.js, `orchester_exec::launch`) get it unchanged; any other `.cmd`
//!   script is refused before it starts, as it already is for the empty `--setting-sources` value.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use uuid::Uuid;

use super::{InvocationError, agent_command, unicode_folders};
use crate::{
  command::Command,
  connection::{
    claude_approval::{ClaudeApprovalScope, PROMPT_TOOL},
    mcp::McpServerName,
  },
  permission::{
    capability::{EnforcedPermissions, Mechanism, PermissionSetting},
    claude::{PermissionFolders, edits_allowed_by_rule, permission_args},
  },
  resource::CommandEnvironment,
  run::STEP_RESULT_SCHEMA,
};

/// [`STEP_RESULT_SCHEMA`] on one line, as `--json-schema` takes it.
pub fn result_schema() -> String {
  let schema: serde_json::Value =
    serde_json::from_str(STEP_RESULT_SCHEMA).expect("the static step result schema is valid JSON");
  schema.to_string()
}

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
  /// The extra writable folders the permissions grant, resolved
  /// ([`crate::permission::WritableFolders::resolve`]).
  pub writable_roots: &'a [PathBuf],
  pub worktree: &'a Path,
  pub session: ClaudeSession<'a>,
  /// The MCP servers the invocation gives the agent: the file that configures them and their names.
  pub mcp: Option<(&'a Path, &'a [McpServerName])>,
  /// The folder holding the skills the agent gets, if it gets any.
  pub skills: Option<&'a Path>,
  /// The code index folder the agent gets, if it gets one (`crate::code_index`).
  pub code_index: Option<&'a Path>,
  /// The configuration file of the permission prompt relay, for permissions that ask a person.
  pub approvals: Option<&'a Path>,
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
    args.extend(["--json-schema".to_owned(), result_schema()]);
    let configs: Vec<&Path> = self
      .mcp
      .map(|(config, _)| config)
      .into_iter()
      .chain(self.approvals)
      .collect();
    if !configs.is_empty() {
      args.push("--mcp-config".to_owned());
      args.extend(configs.iter().map(|config| config.to_string_lossy().into_owned()));
    }
    if self.approvals.is_some() {
      args.extend(["--permission-prompt-tool".to_owned(), PROMPT_TOOL.to_owned()]);
    }
    let servers = self.mcp.map_or(&[][..], |(_, servers)| servers);
    let granted = if self.permissions.writable_folders().is_some() {
      self.writable_roots
    } else {
      &[]
    };
    let editable: Vec<&Path> = std::iter::once(self.worktree)
      .chain(granted.iter().map(PathBuf::as_path))
      .collect();
    let protected: Vec<&Path> = self.code_index.into_iter().collect();
    let folders = PermissionFolders {
      editable: &editable,
      protected: &protected,
    };
    args.extend(permission_args(self.permissions, servers, folders));
    for root in granted {
      args.extend(["--add-dir".to_owned(), root.to_string_lossy().into_owned()]);
    }
    for folder in [self.skills, self.code_index].into_iter().flatten() {
      args.extend(["--add-dir".to_owned(), folder.to_string_lossy().into_owned()]);
    }
    match self.session {
      ClaudeSession::New(id) => args.extend(["--session-id".to_owned(), id.to_string()]),
      ClaudeSession::Resume(session) => args.extend(["--resume".to_owned(), session.to_owned()]),
    }
    args.into_iter().map(OsString::from).collect()
  }

  /// Refuses a step that edits by rule when a folder it may edit gets no rule: every edit there
  /// would be denied without a word.
  fn edit_rules_for_every_folder(&self) -> Result<(), InvocationError> {
    if self.permissions.mechanism(PermissionSetting::Repository) != Mechanism::ClaudeEditRules {
      return Ok(());
    }
    let granted = self
      .permissions
      .writable_folders()
      .map_or(&[][..], |_| self.writable_roots);
    let editable = std::iter::once(self.worktree).chain(granted.iter().map(PathBuf::as_path));
    match editable.into_iter().find(|folder| !edits_allowed_by_rule(folder)) {
      Some(folder) => Err(InvocationError::NoEditRule(folder.to_path_buf())),
      None => Ok(()),
    }
  }

  /// The command that runs the step: in the worktree, with `prompt` on standard input, the
  /// allowed part of `environment` and the variables the project adds (`project`).
  pub fn command(
    &self,
    prompt: String,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    if ClaudeApprovalScope::of(self.permissions.permissions(), &[]).is_some() && self.approvals.is_none() {
      return Err(InvocationError::NoPermissionPrompt);
    }
    unicode_folders(self.writable_roots)?;
    self.edit_rules_for_every_folder()?;
    let read_only: Vec<PathBuf> = [self.skills, self.code_index]
      .into_iter()
      .flatten()
      .map(Path::to_path_buf)
      .collect();
    unicode_folders(&read_only)?;
    agent_command(
      self.executable,
      self.args(),
      self.worktree,
      prompt,
      (environment, project, &[]),
    )
  }
}
