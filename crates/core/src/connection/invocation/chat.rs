//! How a turn of the project's Orchestrator chat (`crate::orchestrator`) starts the Claude Code CLI:
//!
//! `claude -p --input-format stream-json --output-format stream-json --verbose --replay-user-messages
//!  --model <model> [--effort <e>]
//!  --strict-mcp-config --setting-sources "" --disable-slash-commands --system-prompt-file <file>
//!  --mcp-config <file> --tools Read,Glob,Grep --allowedTools mcp__orchester
//!  --disallowedTools Read(//<denied folder>/**)… --permission-mode default --max-turns 30
//!  (--session-id <new uuid> | --resume <session>)`
//!
//! in the project folder, with the person's message as the one stream-json user message on standard input, which is
//! then closed: the CLI exits once it has answered.
//!
//! - Read-only, in the project folder only: the only built-in tools it is offered are `Read`, `Glob` and `Grep`, so it
//!   has no shell and no edit tool at all. They get no allow rule, since a bare tool name would allow every path the
//!   person can read: in the `default` permission mode a read in the working folder runs, one outside it would ask,
//!   and print mode denies what it would ask. The folders that hold secrets are denied by rule as well (`denied`: the
//!   runner's private files, which hold other invocations' MCP tokens, and the account's own CLI folder).
//! - Its only MCP server is Orchester's domain tools for this project (`orchester`), from a file the runner writes for
//!   the turn with the chat's bearer token in it ([`super::ClaudeMcpConfig`]); its tools run without asking: they check
//!   every action themselves, as for any client of the app.
//! - Isolation from the user's own setup as for a step ([`super::ClaudeInvocation`]): no settings file, no slash
//!   command, no MCP server but its own.
//! - The system prompt (`crate::orchestrator::instructions`) is a file given on every turn, a resumed one too.
//! - Sessions are kept on disk so that the next turn resumes them; no step result schema is asked for.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use super::{ClaudeSession, InvocationError, agent_environment, input_line};
use crate::{
  command::Command,
  orchestrator::{MAX_TURNS, instructions::SERVER},
  permission::claude::read_rule,
  resource::CommandEnvironment,
};

/// The built-in tools the Orchestrator's agent is offered: reading and searching, nothing that changes or runs.
const READ_TOOLS: [&str; 3] = ["Read", "Glob", "Grep"];

/// Everything that decides how one turn of the Orchestrator chat starts Claude Code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaudeChat<'a> {
  pub executable: &'a Path,
  pub model: &'a str,
  /// `None` leaves the CLI's default.
  pub effort: Option<&'a str>,
  /// The project folder, the working directory.
  pub folder: &'a Path,
  pub session: ClaudeSession<'a>,
  /// The file with the system prompt.
  pub system_prompt: &'a Path,
  /// The `--mcp-config` file of the domain tools.
  pub mcp_config: &'a Path,
  /// Folders the agent must not read even where the CLI would ask: they hold secrets.
  pub denied: &'a [PathBuf],
}

impl ClaudeChat<'_> {
  /// Arguments after the program, in order; the same on Windows and macOS.
  pub fn args(&self) -> Vec<String> {
    let mut args: Vec<String> = [
      "-p",
      "--input-format",
      "stream-json",
      "--output-format",
      "stream-json",
      "--verbose",
      "--replay-user-messages",
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
    args.extend(["--system-prompt-file".to_owned(), path_text(self.system_prompt)]);
    args.extend(["--mcp-config".to_owned(), path_text(self.mcp_config)]);
    args.extend(["--tools".to_owned(), READ_TOOLS.join(",")]);
    args.extend(["--allowedTools".to_owned(), format!("mcp__{SERVER}")]);
    let denied: Vec<String> = self.denied.iter().filter_map(|folder| read_rule(folder)).collect();
    if !denied.is_empty() {
      args.push("--disallowedTools".to_owned());
      args.extend(denied);
    }
    args.extend(["--permission-mode".to_owned(), "default".to_owned()]);
    args.extend(["--max-turns".to_owned(), MAX_TURNS.to_string()]);
    match self.session {
      ClaudeSession::New(id) => args.extend(["--session-id".to_owned(), id.to_string()]),
      ClaudeSession::Resume(session) => args.extend(["--resume".to_owned(), session.to_owned()]),
    }
    args
  }

  /// The command of the turn that answers `message`: in the project folder, with the allowed part of `environment`
  /// and the variables the project adds (`project`).
  pub fn command(
    &self,
    message: &str,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
    project: &CommandEnvironment,
  ) -> Result<Command, InvocationError> {
    for path in [self.folder, self.system_prompt, self.mcp_config] {
      if path.to_str().is_none() {
        return Err(InvocationError::NotUnicode(path.to_path_buf()));
      }
    }
    let mut variables = agent_environment(environment);
    project.add_to(&mut variables);
    Ok(
      Command::new(self.executable, self.args())
        .with_current_dir(self.folder)
        .with_environment(variables)
        .with_stdin(input_line(message)),
    )
  }
}

/// A path the caller checked to be Unicode.
fn path_text(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}
