//! Auto mode's reviewers on the project's agent connections ([`crate::run::review`]). Each one gets the
//! same instructions and message ([`crate::run::review::review_prompt`], the agent's text as data it
//! cannot break out of) and gives the same strict JSON verdict; the hard rules come before any of them,
//! and every failure asks the person.
//!
//! - Claude Code ([`ClaudeReviewer`]) in print mode, isolated from everything an agent gets: no tools (`--tools ""`),
//!   no MCP server (`--strict-mcp-config` without a configuration), no settings, skills, slash commands or auto memory
//!   ([`crate::connection::invocation::without_auto_memory`]), no session kept; the instructions as the system prompt
//!   (`--system-prompt-file`), the message on standard input, and the verdict as structured output (`--json-schema`,
//!   read from the `structured_output` of the result that ends its `--output-format stream-json` output). The same
//!   output carries what the review spent (the result's `usage`) and the account's plan limits (`rate_limit_event`),
//!   which `--output-format json` would leave out.
//! - Codex ([`CodexReviewer`]): `codex exec` in its read-only sandbox, see [`codex`].
//! - A native connection on the Anthropic API: one Messages request without tools, see [`native`].
//!
//! The CLIs run in an empty folder the runner owns, which the runner removes afterwards, and the runner
//! bounds how long a review may take. What a review spent is read from whatever the CLI printed, also when it
//! failed; a CLI that was stopped, timed out or printed no usage it could read spent an unknown amount
//! ([`ReviewSpend::Unknown`]), and one that could not be started spent nothing.
//!
//! The story-point estimator runs on the same connection and model in the same isolation, with its own
//! instructions and schema ([`estimate`]).

mod codex;
mod estimate;
mod native;

use std::path::{Path, PathBuf};

use serde_json::Value;

pub use self::{
  codex::{CodexReviewer, codex_reviewer_args, parse_codex_review, read_codex_review},
  estimate::{
    CliEstimator, Estimated, EstimatorCli, claude_estimate_args, native_estimate_request, read_claude_estimate,
    read_codex_estimate,
  },
  native::{NATIVE_MAX_OUTPUT, NativeSpend, native_request, native_verdict},
};
use crate::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{invocation::without_auto_memory, plan_limits::claude_limits, stream::claude_usage},
  resource::ModelId,
  run::review::{
    DetailRedaction, REVIEW_SCHEMA, RequestReviewer, ReviewError, ReviewInput, ReviewSpend, ReviewVerdict, Reviewed,
    parse_verdict, review_prompt,
  },
};

/// Claude Code's switch for its nonessential traffic (update check, telemetry, error reports).
pub const QUIET: (&str, &str) = ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
/// The most lines of a failing CLI's error output kept as why it failed: its last ones.
const ERROR_LINES: usize = 5;

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
    let command = Command::new(&self.executable, args)
      .with_current_dir(&self.folder.work)
      .with_environment(self.environment.clone())
      // No update check, telemetry or error reports before the one request: they only delay the verdict.
      .with_env(QUIET.0, QUIET.1)
      .with_stdin(review_prompt(input));
    without_auto_memory(command)
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
/// what it spent and the limits it reported also when it failed. Why it gave no verdict holds none of the command's
/// secrets and nothing of its prompt ([`DetailRedaction::of`]).
fn run_review(runner: &impl CommandRunner, command: &Command, read: fn(&str) -> Reviewed) -> Reviewed {
  let mut answer = match run_cli(runner, command) {
    Ok((stdout, exited)) => {
      let mut answer = read(&stdout);
      if let Err(error) = exited {
        answer.verdict = Err(error);
      }
      answer
    }
    Err((error, spend)) => Reviewed::new(Err(error), spend),
  };
  answer.verdict = answer
    .verdict
    .map_err(|error| error.redacted(&DetailRedaction::of(command)));
  answer
}

/// What a one-request CLI (a reviewer, the story-point estimator) printed, and whether it exited successfully; or
/// why it gave nothing, with what it spent. A timeout, a failure to start and a failing exit are errors of the
/// request; a program that never started spent nothing, one that was ended (or that a runner could not say whether
/// it started) spent an unknown amount.
type CliOutput = Result<(String, Result<(), ReviewError>), (ReviewError, ReviewSpend)>;

fn run_cli(runner: &impl CommandRunner, command: &Command) -> CliOutput {
  match runner.run(command) {
    Ok(output) => {
      let exited = if output.success() {
        Ok(())
      } else {
        Err(ReviewError::CouldNotRun(exit_detail(&output)))
      };
      Ok((output.stdout, exited))
    }
    Err(CommandError::TimedOut) => Err((ReviewError::TimedOut, ReviewSpend::Unknown)),
    Err(error @ (CommandError::NotFound | CommandError::NotStarted(_) | CommandError::UnsafeScriptArgument { .. })) => {
      Err((ReviewError::CouldNotRun(error.to_string()), ReviewSpend::Nothing))
    }
    Err(error @ CommandError::Failed(_)) => Err((ReviewError::CouldNotRun(error.to_string()), ReviewSpend::Unknown)),
  }
}

/// Why a CLI exited with a failure: its exit status, and below it the error it reported in its output
/// ([`reported_error`]) or else the last lines of its error output.
fn exit_detail(output: &CommandOutput) -> String {
  let status = output.code.map_or_else(
    || "the CLI was ended without an exit code".to_owned(),
    |code| format!("the CLI exited with code {code}"),
  );
  let said = reported_error(&output.stdout).or_else(|| {
    let lines: Vec<&str> = output
      .stderr
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .collect();
    let last = &lines[lines.len().saturating_sub(ERROR_LINES)..];
    (!last.is_empty()).then(|| last.join("\n"))
  });
  match said {
    Some(said) => format!("{status}:\n{said}"),
    None => status,
  }
}

/// The last error a CLI reported in its output, in its own words: the text of Claude Code's error `result` (signed
/// out: `Not logged in · Please run /login`; at its plan limit, the limit and when it resets), the message of Codex's
/// `turn.failed` or `error` event.
fn reported_error(stdout: &str) -> Option<String> {
  let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_owned);
  events(stdout)
    .filter_map(|event| match kind(&event) {
      Some("result") if event.get("is_error").and_then(Value::as_bool) == Some(true) => {
        text(event.get("result")).or_else(|| text(event.pointer("/errors/0")))
      }
      Some("turn.failed") => text(event.pointer("/error/message")),
      Some("error") => text(event.get("message")),
      _ => None,
    })
    .filter(|said| !said.trim().is_empty())
    .last()
}

/// Whether `word` may be passed as an effort: a plain word, nothing a CLI could read as more.
fn plain_word(word: &str) -> bool {
  !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Claude Code's arguments after the program for a review: [`claude_args`] with the verdict's schema.
pub fn reviewer_args(model: (&ModelId, Option<&str>), instructions: &Path) -> Vec<String> {
  claude_args(model, instructions, REVIEW_SCHEMA)
}

/// Claude Code's arguments after the program: print mode with its stream of events (which needs `--verbose`), the
/// model and its effort (only a plain word is passed), nothing of the user's or an agent's setup, no tools, the
/// system prompt in `instructions`, and the answer's `schema`.
fn claude_args((model, effort): (&ModelId, Option<&str>), instructions: &Path, schema: &str) -> Vec<String> {
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
  args.extend(["--json-schema".to_owned(), schema.to_owned()]);
  args
}

/// The verdict in Claude Code's output: the structured output of its last `result` line.
pub fn parse_review(stdout: &str) -> Result<ReviewVerdict, ReviewError> {
  claude_structured(stdout).and_then(parse_verdict)
}

/// The structured output of Claude Code's last `result` line, unless it is an error, which says why in its words.
fn claude_structured(stdout: &str) -> Result<Value, ReviewError> {
  let unreadable = |detail: &str| ReviewError::Unparseable(detail.to_owned());
  let result = claude_result(stdout).ok_or_else(|| unreadable("no JSON result"))?;
  if result.get("is_error").and_then(Value::as_bool) != Some(false) {
    return Err(match reported_error(stdout) {
      Some(said) => ReviewError::Unparseable(format!("the result is an error:\n{said}")),
      None => unreadable("the result is an error"),
    });
  }
  result
    .get("structured_output")
    .cloned()
    .ok_or_else(|| unreadable("no structured output"))
}

/// Claude Code's answer in its output: the verdict ([`parse_review`]), what the review spent as its last
/// `result` line reports it (for the whole review), and the plan limits of every `rate_limit_event`.
pub fn read_claude_review(stdout: &str) -> Reviewed {
  let limits = events(stdout)
    .filter(|event| kind(event) == Some("rate_limit_event"))
    .filter_map(|event| event.get("rate_limit_info").and_then(claude_limits))
    .collect();
  Reviewed {
    verdict: parse_review(stdout),
    spend: claude_spend(stdout),
    limits,
  }
}

/// What Claude Code's request spent, as its last `result` line reports it.
fn claude_spend(stdout: &str) -> ReviewSpend {
  claude_result(stdout)
    .and_then(|result| result.get("usage").and_then(claude_usage))
    .map_or(ReviewSpend::Unknown, ReviewSpend::Reported)
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
