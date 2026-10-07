//! Auto mode's reviewer on a Codex connection: one `codex exec` turn, isolated from everything an agent
//! gets.
//!
//! `codex -a never exec --json --ephemeral --ignore-user-config --ignore-rules --skip-git-repo-check
//!  --sandbox read-only -m <model> [-c model_reasoning_effort=<effort>] -c notify=[] … -c features.shell_tool=false
//!  -c tools.web_search=false --output-schema <schema file> -C <empty folder> -`
//!
//! - Nothing waits for an approval (`-a never`, a top-level flag), and the sandbox is read-only without network; the
//!   shell tool and web search are turned off, so the model has nothing to run. Nothing of the user's configuration,
//!   execpolicy rules or MCP servers is loaded, and no session is kept.
//! - `exec` has no system prompt of its own, so the instructions come first in the message on standard input (`-`),
//!   before the request and its context, which stay data between their markers.
//! - `--output-schema` makes the final message the verdict's JSON; it is read strictly from the last `agent_message` of
//!   the `--json` events, and a failed turn or an error event is a review that did not run. Which of these flags the
//!   installed Codex takes was not checked against a live CLI (by instruction): one it refuses makes every review fail,
//!   which asks the person.
//! - What the review spent is the `usage` of its `turn.completed` events, OpenAI-style: `input_tokens` holds the
//!   `cached_input_tokens`. `codex exec --json` reports no plan limits (Codex 0.159.2 has no such event in it; App
//!   Server's `account/rateLimits/updated` is not part of `exec`), so a Codex review adds nothing to the plan limits
//!   history.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{ReviewFolder, events, kind, plain_word, run_review};
use crate::{
  command::{Command, CommandRunner},
  connection::invocation::CODEX_ISOLATION as ISOLATION,
  resource::ModelId,
  run::{
    TokenUsage,
    review::{
      REVIEW_INSTRUCTIONS, RequestReviewer, ReviewError, ReviewInput, ReviewSpend, ReviewVerdict, Reviewed,
      parse_verdict_text, review_prompt,
    },
  },
};

/// The reviewer on a Codex connection.
pub struct CodexReviewer<R> {
  executable: PathBuf,
  model: ModelId,
  effort: Option<String>,
  folder: ReviewFolder,
  environment: Vec<(String, String)>,
  runner: R,
}

impl<R: CommandRunner> CodexReviewer<R> {
  /// A reviewer that runs the CLI at `executable` with `model` and `effort` in `folder`, through
  /// `runner`, which stops it when it takes too long.
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
    let args = codex_reviewer_args((&self.model, self.effort.as_deref()), &self.folder);
    Command::new(&self.executable, args)
      .with_current_dir(&self.folder.work)
      .with_environment(self.environment.clone())
      .with_stdin(format!("{REVIEW_INSTRUCTIONS}\n\n{}", review_prompt(input)))
  }
}

impl<R: CommandRunner> RequestReviewer for CodexReviewer<R> {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, input: &ReviewInput) -> Reviewed {
    run_review(&self.runner, &self.command(input), read_codex_review)
  }
}

/// Codex's arguments after the program, for `model` with `effort` (only a plain word is passed) in
/// `folder`.
pub fn codex_reviewer_args((model, effort): (&ModelId, Option<&str>), folder: &ReviewFolder) -> Vec<String> {
  let mut args: Vec<String> = [
    "-a",
    "never",
    "exec",
    "--json",
    "--ephemeral",
    "--ignore-user-config",
    "--ignore-rules",
    "--skip-git-repo-check",
    "--sandbox",
    "read-only",
    "-m",
    model.as_str(),
  ]
  .map(str::to_owned)
  .to_vec();
  if let Some(effort) = effort.filter(|effort| plain_word(effort)) {
    args.extend(["-c".to_owned(), format!("model_reasoning_effort={effort}")]);
  }
  for setting in ISOLATION
    .iter()
    .copied()
    .chain(["features.shell_tool=false", "tools.web_search=false"])
  {
    args.extend(["-c".to_owned(), setting.to_owned()]);
  }
  args.extend([
    "--output-schema".to_owned(),
    folder.schema.display().to_string(),
    "-C".to_owned(),
    folder.work.display().to_string(),
    "-".to_owned(),
  ]);
  args
}

/// The verdict in `codex exec --json`'s events: the text of its last `agent_message`, strictly the JSON
/// object alone. A failed turn or an error event is a review that did not run.
pub fn parse_codex_review(stdout: &str) -> Result<ReviewVerdict, ReviewError> {
  let events: Vec<Value> = events(stdout).collect();
  if let Some(failed) = events
    .iter()
    .find(|event| matches!(kind(event), Some("turn.failed" | "error")))
  {
    return Err(ReviewError::CouldNotRun(format!(
      "Codex reported {}",
      kind(failed).unwrap_or_default()
    )));
  }
  let text = events
    .iter()
    .rev()
    .filter(|event| kind(event) == Some("item.completed"))
    .filter_map(|event| event.get("item"))
    .find(|item| item.get("type").and_then(Value::as_str) == Some("agent_message"))
    .and_then(|item| item.get("text").and_then(Value::as_str))
    .ok_or_else(|| ReviewError::Unparseable("no final message".to_owned()))?;
  parse_verdict_text(text)
}

/// Codex's answer in `codex exec --json`'s events: the verdict ([`parse_codex_review`]) and what the review
/// spent, the `usage` of its `turn.completed` events summed; unknown when none of them reports it readably.
pub fn read_codex_review(stdout: &str) -> Reviewed {
  let turns: Option<Vec<TokenUsage>> = events(stdout)
    .filter(|event| kind(event) == Some("turn.completed"))
    .map(|event| event.get("usage").and_then(codex_usage))
    .collect();
  let spend = match turns {
    Some(turns) if !turns.is_empty() => ReviewSpend::Reported(turns.into_iter().sum()),
    _ => ReviewSpend::Unknown,
  };
  Reviewed::new(parse_codex_review(stdout), spend)
}

/// The `usage` of a `turn.completed` event: input with its cached part, and output with reasoning.
fn codex_usage(usage: &Value) -> Option<TokenUsage> {
  let count = |name: &str| usage.get(name).and_then(Value::as_u64);
  Some(TokenUsage {
    input: count("input_tokens")?,
    cached_input: usage.get("cached_input_tokens").map_or(Some(0), Value::as_u64)?,
    output: count("output_tokens")?,
  })
}
