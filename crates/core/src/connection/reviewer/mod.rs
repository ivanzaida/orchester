//! Auto mode's reviewers on the project's agent connections ([`crate::run::review`]). Each one gets the
//! same instructions and message ([`crate::run::review::review_prompt`], the agent's text as data it
//! cannot break out of) and gives the same strict JSON verdict; the hard rules come before any of them,
//! and every failure asks the person.
//!
//! - Claude Code ([`ClaudeReviewer`]) in print mode, isolated from everything an agent gets: no tools (`--tools ""`),
//!   no MCP server (`--strict-mcp-config` without a configuration), no settings, skills or slash commands, no session
//!   kept; the instructions as the system prompt (`--system-prompt-file`), the message on standard input, and the
//!   verdict as structured output (`--json-schema`, read from the `structured_output` of the `--output-format json`
//!   result).
//! - Codex ([`CodexReviewer`]): `codex exec` in its read-only sandbox, see [`codex`].
//! - A native connection on the Anthropic API: one Messages request without tools, see [`native`].
//!
//! The CLIs run in an empty folder the runner owns, which the runner removes afterwards, and the runner
//! bounds how long a review may take.

mod codex;
mod native;

use std::path::{Path, PathBuf};

use serde_json::Value;

pub use self::{
  codex::{CodexReviewer, codex_reviewer_args, parse_codex_review},
  native::{NATIVE_MAX_OUTPUT, native_request, native_verdict},
};
use crate::{
  command::{Command, CommandError, CommandRunner},
  resource::ModelId,
  run::review::{
    REVIEW_SCHEMA, RequestReviewer, ReviewError, ReviewInput, ReviewVerdict, parse_verdict, review_prompt,
  },
};

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
      .with_stdin(review_prompt(input))
  }
}

impl<R: CommandRunner> RequestReviewer for ClaudeReviewer<R> {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, input: &ReviewInput) -> Result<ReviewVerdict, ReviewError> {
    let output = run(&self.runner, &self.command(input))?;
    parse_review(&output)
  }
}

/// `command`'s standard output once it exited successfully; a timeout, a failure to start and a failing
/// exit are errors of the review.
fn run(runner: &impl CommandRunner, command: &Command) -> Result<String, ReviewError> {
  let output = runner.run(command).map_err(|error| match error {
    CommandError::TimedOut => ReviewError::TimedOut,
    other => ReviewError::CouldNotRun(other.to_string()),
  })?;
  if !output.success() {
    return Err(ReviewError::CouldNotRun(format!(
      "the CLI exited with {:?}",
      output.code
    )));
  }
  Ok(output.stdout)
}

/// Whether `word` may be passed as an effort: a plain word, nothing a CLI could read as more.
fn plain_word(word: &str) -> bool {
  !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Claude Code's arguments after the program: print mode, the model and its effort (only a plain word is
/// passed), nothing of the user's or an agent's setup, no tools, the system prompt in `instructions`, and
/// the verdict's schema.
pub fn reviewer_args((model, effort): (&ModelId, Option<&str>), instructions: &Path) -> Vec<String> {
  let mut args: Vec<String> = [
    "-p",
    "--output-format",
    "json",
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

/// The verdict in Claude Code's `--output-format json` result: its last `result` line.
pub fn parse_review(stdout: &str) -> Result<ReviewVerdict, ReviewError> {
  let unreadable = |detail: &str| ReviewError::Unparseable(detail.to_owned());
  let result: Value = stdout
    .lines()
    .rev()
    .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
    .find(|value| value.get("type").and_then(Value::as_str) == Some("result"))
    .ok_or_else(|| unreadable("no JSON result"))?;
  if result.get("is_error").and_then(Value::as_bool) != Some(false) {
    return Err(unreadable("the result is an error"));
  }
  let structured = result
    .get("structured_output")
    .cloned()
    .ok_or_else(|| unreadable("no structured output"))?;
  parse_verdict(structured)
}
