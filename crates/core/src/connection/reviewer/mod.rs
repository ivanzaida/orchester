//! Auto mode's reviewers on the project's agent connections ([`crate::run::review`]). Each one gets the
//! same instructions and message ([`crate::run::review::review_prompt`], the agent's text as data it
//! cannot break out of) and gives the same strict JSON verdict; the hard rules come before any of them,
//! and every failure asks the person.
//!
//! - Claude Code ([`ClaudeReviewer`]) in print mode, isolated from everything an agent gets: no tools (`--tools ""`),
//!   no MCP server (`--strict-mcp-config` without a configuration), no settings, skills or slash commands, no session
//!   kept; the instructions as the system prompt (`--system-prompt-file`), the message on standard input, and the
//!   verdict as structured output (`--json-schema`, read from the `structured_output` of the result that ends its
//!   `--output-format stream-json` output). The same output carries what the review spent (the result's `usage`) and
//!   the account's plan limits (`rate_limit_event`), which `--output-format json` would leave out.
//! - Codex ([`CodexReviewer`]): `codex exec` in its read-only sandbox, see [`codex`].
//! - A native connection on the Anthropic API: one Messages request without tools, see [`native`].
//!
//! The CLIs run in an empty folder the runner owns, which the runner removes afterwards, and the runner
//! bounds how long a review may take. What a review spent is read from whatever the CLI printed, also when it
//! failed; a CLI that was stopped, timed out or printed no usage it could read spent an unknown amount
//! ([`ReviewSpend::Unknown`]), and one that could not be started spent nothing.

mod codex;
mod native;

use std::path::{Path, PathBuf};

use serde_json::Value;

pub use self::{
  codex::{CodexReviewer, codex_reviewer_args, parse_codex_review, read_codex_review},
  native::{NATIVE_MAX_OUTPUT, native_request, native_review, native_verdict},
};
use crate::{
  command::{Command, CommandError, CommandRunner},
  connection::{plan_limits::claude_limits, stream::claude_usage},
  resource::ModelId,
  run::review::{
    REVIEW_SCHEMA, RequestReviewer, ReviewError, ReviewInput, ReviewSpend, ReviewVerdict, Reviewed, parse_verdict,
    review_prompt,
  },
};

/// Claude Code's switch for its nonessential traffic (update check, telemetry, error reports).
pub const QUIET: (&str, &str) = ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");

/// Where a CLI reviewer runs: an empty folder, and the files it reads beside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewFolder {
  /// The empty working folder.
  pub work: PathBuf,
  /// The file holding [`crate::run::review::REVIEW_INSTRUCTIONS`].
  pub instructions: PathBuf,
  /// The file holding [`REVIEW_SCHEMA`].
  pub schema: PathBuf,
}

/// The reviewer on a Claude Code connection.
pub struct ClaudeReviewer<R> {
  executable: PathBuf,
  model: ModelId,
  effort: Option<String>,
  folder: ReviewFolder,
  /// The variables the CLI starts with: the agent environment and the project's own.
  environment: Vec<(String, String)>,
  runner: R,
}

impl<R: CommandRunner> ClaudeReviewer<R> {
  /// A reviewer that runs the CLI at `executable` with `model` and `effort` in `folder`, through `runner`,
  /// which stops it when it takes too long.
  pub fn new(
    executable: &Path,
    (model, effort): (ModelId, Option<String>),
    (folder, environment): (ReviewFolder, Vec<(String, String)>),
    runner: R,
  ) -> Self {
    Self {
      executable: executable.to_path_buf(),
      model,
      effort,
      folder,
      environment,
      runner,
    }
  }

  /// The command that reviews `input`.
  pub fn command(&self, input: &ReviewInput) -> Command {
    let args = reviewer_args((&self.model, self.effort.as_deref()), &self.folder.instructions);
    Command::new(&self.executable, args)
      .with_current_dir(&self.folder.work)
      .with_environment(self.environment.clone())
      // No update check, telemetry or error reports before the one request: they only delay the verdict.
      .with_env(QUIET.0, QUIET.1)
      .with_stdin(review_prompt(input))
  }
}

impl<R: CommandRunner> RequestReviewer for ClaudeReviewer<R> {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, input: &ReviewInput) -> Reviewed {
    run_review(&self.runner, &self.command(input), read_claude_review)
  }
}

/// `command`'s answer as `read` finds it in its standard output: the verdict once it exited successfully, and
/// what it spent and the limits it reported also when it failed. A timeout, a failure to start and a failing
/// exit are errors of the review; a program that never started spent nothing, one that was ended spent an
/// unknown amount.
fn run_review(runner: &impl CommandRunner, command: &Command, read: fn(&str) -> Reviewed) -> Reviewed {
  match runner.run(command) {
    Ok(output) => {
      let mut answer = read(&output.stdout);
      if !output.success() {
        answer.verdict = Err(ReviewError::CouldNotRun(format!(
          "the CLI exited with {:?}",
          output.code
        )));
      }
      answer
    }
    Err(CommandError::TimedOut) => Reviewed::new(Err(ReviewError::TimedOut), ReviewSpend::Unknown),
    Err(error @ (CommandError::NotFound | CommandError::UnsafeScriptArgument { .. })) => {
      Reviewed::new(Err(ReviewError::CouldNotRun(error.to_string())), ReviewSpend::Nothing)
    }
    Err(error @ CommandError::Failed(_)) => {
      Reviewed::new(Err(ReviewError::CouldNotRun(error.to_string())), ReviewSpend::Unknown)
    }
  }
}

/// Whether `word` may be passed as an effort: a plain word, nothing a CLI could read as more.
fn plain_word(word: &str) -> bool {
  !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Claude Code's arguments after the program: print mode with its stream of events (which needs `--verbose`), the
/// model and its effort (only a plain word is passed), nothing of the user's or an agent's setup, no tools, the
/// system prompt in `instructions`, and the verdict's schema.
pub fn reviewer_args((model, effort): (&ModelId, Option<&str>), instructions: &Path) -> Vec<String> {
  let mut args: Vec<String> = [
    "-p",
    "--output-format",
    "stream-json",
    "--verbose",
    "--model",
    model.as_str(),
    "--strict-mcp-config",
    "--setting-sources",
    "",
    "--disable-slash-commands",
    "--no-session-persistence",
    "--tools",
    "",
  ]
  .map(str::to_owned)
  .to_vec();
  if let Some(effort) = effort.filter(|effort| plain_word(effort)) {
    args.extend(["--effort".to_owned(), effort.to_owned()]);
  }
  args.extend(["--system-prompt-file".to_owned(), instructions.display().to_string()]);
  args.extend(["--json-schema".to_owned(), REVIEW_SCHEMA.to_owned()]);
  args
}

/// The verdict in Claude Code's output: the structured output of its last `result` line.
pub fn parse_review(stdout: &str) -> Result<ReviewVerdict, ReviewError> {
  let unreadable = |detail: &str| ReviewError::Unparseable(detail.to_owned());
  let result = claude_result(stdout).ok_or_else(|| unreadable("no JSON result"))?;
  if result.get("is_error").and_then(Value::as_bool) != Some(false) {
    return Err(unreadable("the result is an error"));
  }
  let structured = result
    .get("structured_output")
    .cloned()
    .ok_or_else(|| unreadable("no structured output"))?;
  parse_verdict(structured)
}

/// Claude Code's answer in its output: the verdict ([`parse_review`]), what the review spent as its last
/// `result` line reports it (for the whole review), and the plan limits of every `rate_limit_event`.
pub fn read_claude_review(stdout: &str) -> Reviewed {
  let spend = claude_result(stdout)
    .and_then(|result| result.get("usage").and_then(claude_usage))
    .map_or(ReviewSpend::Unknown, ReviewSpend::Reported);
  let limits = events(stdout)
    .filter(|event| kind(event) == Some("rate_limit_event"))
    .filter_map(|event| event.get("rate_limit_info").and_then(claude_limits))
    .collect();
  Reviewed {
    verdict: parse_review(stdout),
    spend,
    limits,
  }
}

/// The last `result` line of Claude Code's output.
fn claude_result(stdout: &str) -> Option<Value> {
  events(stdout).filter(|event| kind(event) == Some("result")).last()
}

/// The JSON objects of a CLI's output, one per line; other lines are left out.
fn events(stdout: &str) -> impl Iterator<Item = Value> + '_ {
  stdout
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
}

/// An event's `type`.
fn kind(event: &Value) -> Option<&str> {
  event.get("type").and_then(Value::as_str)
}
