//! Reading an account's plan limits outside a run (`docs/design/agent-accounts.md`, "Plan limits read on their
//! own"). Orchester runs only the account's CLI with the account's folder variable; it never reads, copies or sends
//! a credential.
//!
//! - Codex: `codex app-server --stdio` with the isolation overrides agents get, asked `initialize`, `initialized` and
//!   `account/rateLimits/read`. No thread or turn starts, so no token is spent. App Server stops at the end of its
//!   input without answering what it was still working on (Codex 0.137.0, checked 2026-10-20 with an empty scratch
//!   `CODEX_HOME`), so the input stays open until the answer is in ([`LineExchange`]).
//! - Claude Code has no command that reads plan usage without a turn (2.1.296: `claude --help`, `claude auth --help`;
//!   `/usage` is a slash command of an interactive session). The person chose (2026-10-20) an explicit *Check now* per
//!   account: the smallest print-mode turn on a cheap model, isolated as auto mode's reviewer is, whose
//!   `rate_limit_event` is the reading. It spends a few tokens of the account's plan, so it never runs by itself.

use std::path::Path;

use serde_json::{Value, json};
use thiserror::Error;

use super::{PlanLimits, claude_limits, codex_read_limits};
use crate::{
  command::{Command, CommandError, CommandOutput, LineExchange},
  connection::{
    invocation::{CODEX_ISOLATION, without_auto_memory},
    reviewer::QUIET,
  },
};

/// The id of the `account/rateLimits/read` request.
const READ_REQUEST: u64 = 2;
/// How much of a CLI's refusal is kept: one line of it.
const REASON_CHARS: usize = 200;

/// The model of Claude Code's *Check now*: the cheapest one its aliases name.
pub const CLAUDE_CHECK_MODEL: &str = "haiku";
/// The whole system prompt of *Check now*, in place of Claude Code's own.
pub const CLAUDE_CHECK_SYSTEM: &str = "Reply with one word.";
/// What *Check now* asks: an answer of a word or two.
pub const CLAUDE_CHECK_PROMPT: &str = "Say OK.";

/// Why a read gave no reading.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ReadError {
  /// Nobody is signed in to the account's folder: there is nothing to read until someone is.
  #[error("nobody is signed in to the account")]
  NotSignedIn,
  /// The provider holds requests back for now: the read waits longer before the next try.
  #[error("the provider is holding requests back: {0}")]
  RateLimited(String),
  #[error("the CLI refused the read: {0}")]
  Refused(String),
  /// The CLI answered, but with nothing about the plan's limits (an API key has none).
  #[error("the CLI reported no plan limits")]
  NoLimits,
  #[error("the CLI ended without answering")]
  NoAnswer,
  #[error(transparent)]
  Command(#[from] CommandError),
}

/// Codex App Server for a read: the CLI at `program` with `environment` (the agent environment with the account's
/// `CODEX_HOME`), the isolation overrides, and the three requests waiting on its input.
pub fn codex_read_command(program: &Path, environment: Vec<(String, String)>) -> Command {
  let mut args = vec!["app-server".to_owned(), "--stdio".to_owned()];
  for setting in CODEX_ISOLATION {
    args.extend(["-c".to_owned(), setting.to_owned()]);
  }
  let requests = [
    json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"orchester","version":"0.1"}}}),
    json!({"method":"initialized"}),
    json!({"id":READ_REQUEST,"method":"account/rateLimits/read"}),
  ];
  let input: String = requests.iter().map(|request| format!("{request}\n")).collect();
  Command::new(program, args)
    .with_environment(environment)
    .with_interactive_input(input)
}

/// Reads the plan limits with `command` ([`codex_read_command`]) through `runner`.
pub fn read_codex_limits(runner: &impl LineExchange, command: &Command) -> Result<PlanLimits, ReadError> {
  let lines = runner.exchange(command, &mut |line| answer(line).is_some())?;
  let answer = lines.iter().find_map(|line| answer(line)).ok_or(ReadError::NoAnswer)?;
  if let Some(error) = answer.get("error") {
    return Err(refusal(
      error.get("message").and_then(Value::as_str).unwrap_or_default(),
    ));
  }
  answer
    .get("result")
    .and_then(codex_read_limits)
    .ok_or(ReadError::NoLimits)
}

/// The answer to the read in `line`, if it is that.
fn answer(line: &str) -> Option<Value> {
  serde_json::from_str::<Value>(line.trim()).ok().filter(|message| {
    message.get("id").and_then(Value::as_u64) == Some(READ_REQUEST) && message.get("method").is_none()
  })
}

/// What a CLI's refusal `message` says, on one line: nobody signed in (App Server's `codex account authentication
/// required to read rate limits`, Claude Code's `Not logged in · Please run /login`), the provider holding requests
/// back, or another refusal in its own words.
fn refusal(message: &str) -> ReadError {
  let line: String = message
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .chars()
    .take(REASON_CHARS)
    .collect();
  let lowered = line.to_lowercase();
  if ["authentication required", "not logged in", "/login"]
    .iter()
    .any(|words| lowered.contains(words))
  {
    ReadError::NotSignedIn
  } else if lowered.contains("429") || lowered.contains("too many requests") {
    ReadError::RateLimited(line)
  } else {
    ReadError::Refused(line)
  }
}

/// Claude Code's *Check now* turn: print mode with its stream of events (which needs `--verbose`) on
/// [`CLAUDE_CHECK_MODEL`], with nothing of the user's or an agent's setup, no tools, no session kept and
/// [`CLAUDE_CHECK_SYSTEM`] as the whole system prompt; the prompt on standard input. It runs in `folder`, an empty
/// one, with `environment` (the agent environment with the account's `CLAUDE_CONFIG_DIR`).
pub fn claude_check_command(program: &Path, environment: Vec<(String, String)>, folder: &Path) -> Command {
  let args = [
    "-p",
    "--output-format",
    "stream-json",
    "--verbose",
    "--model",
    CLAUDE_CHECK_MODEL,
    "--strict-mcp-config",
    "--setting-sources",
    "",
    "--disable-slash-commands",
    "--no-session-persistence",
    "--tools",
    "",
    "--system-prompt",
    CLAUDE_CHECK_SYSTEM,
  ];
  let command = Command::new(program, args)
    .with_current_dir(folder)
    .with_environment(environment)
    .with_env(QUIET.0, QUIET.1)
    .with_stdin(CLAUDE_CHECK_PROMPT.to_owned());
  without_auto_memory(command)
}

/// The plan limits of every `rate_limit_event` in `output` of [`claude_check_command`], in order. A turn that
/// reported none says why when its result does: nobody signed in, or another error in its own words.
pub fn read_claude_check(output: &CommandOutput) -> Result<Vec<PlanLimits>, ReadError> {
  let events: Vec<Value> = output
    .stdout
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
    .collect();
  let kind = |event: &Value| event.get("type").and_then(Value::as_str).map(str::to_owned);
  let limits: Vec<PlanLimits> = events
    .iter()
    .filter(|event| kind(event).as_deref() == Some("rate_limit_event"))
    .filter_map(|event| event.get("rate_limit_info").and_then(claude_limits))
    .collect();
  if !limits.is_empty() {
    return Ok(limits);
  }
  let result = events
    .iter()
    .rev()
    .find(|event| kind(event).as_deref() == Some("result"));
  let Some(result) = result else {
    return Err(ReadError::NoAnswer);
  };
  if result.get("is_error").and_then(Value::as_bool) != Some(true) {
    return Err(ReadError::NoLimits);
  }
  Err(refusal(
    result.get("result").and_then(Value::as_str).unwrap_or_default(),
  ))
}
