//! Auto mode's reviewer on Claude Code ([`crate::run::review`]): the project's Claude Code connection in
//! print mode, isolated from everything an agent gets. No tools (`--tools ""`), no MCP server
//! (`--strict-mcp-config` without a configuration), no settings, skills or slash commands, no session
//! kept; the prompt on standard input, and the verdict as structured output (`--json-schema`, read from
//! the `structured_output` of the `--output-format json` result). The runner bounds how long it may take.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::{
  command::{Command, CommandError, CommandRunner},
  permission::{NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::review::{RequestReviewer, ReviewError, ReviewInput, ReviewVerdict},
};

/// The verdict the reviewer must give, as `--json-schema` takes it.
pub const REVIEW_SCHEMA: &str = r#"{"type":"object","properties":{"verdict":{"type":"string","enum":["approve","ask"]},"reason":{"type":"string","minLength":1}},"required":["verdict","reason"],"additionalProperties":false}"#;

/// What the reviewer is told to do; the request and its context follow.
const INSTRUCTIONS: &str = "You review one tool request of a coding agent that works on the task below, in its own \
git worktree, for Orchester. Decide whether to approve it without asking the person.\n\
Approve only when the request clearly serves the task and the step, and it is none of these:\n\
- destructive: deleting or overwriting outside the worktree, force-pushing, rewriting history beyond the run's own \
branch, dropping data;\n\
- exfiltrating: sending files, code or secrets anywhere over the network;\n\
- escaping: writing outside the folders the step may write, changing credentials, permissions or system settings;\n\
- installing anything persistent (global packages, services, scheduled jobs, shell profiles).\n\
When you are unsure, or the request is not needed for the task, answer \"ask\". Give one short sentence of reason \
that a person can check. Answer only with the JSON the schema asks for.";

/// The reviewer on a Claude Code connection.
pub struct ClaudeReviewer<R> {
  executable: PathBuf,
  model: ModelId,
  worktree: PathBuf,
  /// The variables the CLI starts with: the agent environment and the project's own.
  environment: Vec<(String, String)>,
  runner: R,
}

impl<R: CommandRunner> ClaudeReviewer<R> {
  /// A reviewer that runs the CLI at `executable` with `model` in `worktree`, through `runner`, which
  /// stops it when it takes too long.
  pub fn new(
    executable: &Path,
    model: ModelId,
    (worktree, environment): (&Path, Vec<(String, String)>),
    runner: R,
  ) -> Self {
    Self {
      executable: executable.to_path_buf(),
      model,
      worktree: worktree.to_path_buf(),
      environment,
      runner,
    }
  }

  /// The command that reviews `input`.
  pub fn command(&self, input: &ReviewInput) -> Command {
    Command::new(&self.executable, reviewer_args(&self.model))
      .with_current_dir(&self.worktree)
      .with_environment(self.environment.clone())
      .with_stdin(review_prompt(input))
  }
}

impl<R: CommandRunner> RequestReviewer for ClaudeReviewer<R> {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, input: &ReviewInput) -> Result<ReviewVerdict, ReviewError> {
    let output = self.runner.run(&self.command(input)).map_err(|error| match error {
      CommandError::TimedOut => ReviewError::TimedOut,
      other => ReviewError::CouldNotRun(other.to_string()),
    })?;
    if !output.success() {
      return Err(ReviewError::CouldNotRun(format!(
        "the CLI exited with {:?}",
        output.code
      )));
    }
    parse_review(&output.stdout)
  }
}

/// The reviewer's arguments after the program: print mode, the model, nothing of the user's or an agent's
/// setup, no tools, and the verdict's schema.
pub fn reviewer_args(model: &ModelId) -> Vec<String> {
  [
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
    "--json-schema",
    REVIEW_SCHEMA,
  ]
  .map(str::to_owned)
  .to_vec()
}

/// The prompt: the instructions, then the request, the step, the task and the agent's last messages.
pub fn review_prompt(input: &ReviewInput) -> String {
  let request = input.request();
  let step = input.step();
  let task = input.task();
  let mut prompt = format!("{INSTRUCTIONS}\n\n# Request\n");
  if request.command {
    prompt.push_str(&format!("A shell command: {}\n", request.action));
  } else {
    prompt.push_str(&format!(
      "The tool `{}` of the MCP server `{}`.\n",
      request.action, request.server
    ));
  }
  for (label, value) in [
    ("Arguments", &request.arguments),
    ("The agent's note", &request.note),
    ("Working folder", &request.working_folder),
  ] {
    if let Some(value) = value {
      prompt.push_str(&format!("{label}: {value}\n"));
    }
  }
  let repository = match step.repository {
    RepositoryAccess::ReadOnly => "reads the repository only",
    RepositoryAccess::Write => "may write in its worktree",
  };
  let network = match step.network {
    NetworkAccess::On => "on",
    NetworkAccess::Off => "off",
  };
  prompt.push_str(&format!(
    "\n# Step\n{}: {repository}; network {network}; writable folders besides the worktree: {}\n",
    step.name,
    if step.writable_folders.is_empty() {
      "none".to_owned()
    } else {
      step.writable_folders.join(", ")
    }
  ));
  prompt.push_str(&format!("\n# Task\n{}\n{}\n", task.title, task.brief));
  for criterion in &task.criteria {
    prompt.push_str(&format!("- {criterion}\n"));
  }
  if !input.messages().is_empty() {
    prompt.push_str("\n# The agent's last messages\n");
    for message in input.messages() {
      prompt.push_str(&format!("---\n{message}\n"));
    }
  }
  prompt
}

/// The verdict in the CLI's `--output-format json` result.
pub fn parse_review(stdout: &str) -> Result<ReviewVerdict, ReviewError> {
  #[derive(Deserialize)]
  #[serde(rename_all = "snake_case")]
  enum Verdict {
    Approve,
    Ask,
  }
  #[derive(Deserialize)]
  struct Answer {
    verdict: Verdict,
    reason: String,
  }
  let unreadable = |detail: &str| ReviewError::Unparseable(detail.to_owned());
  let result: Value = stdout
    .lines()
    .rev()
    .find_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
    .ok_or_else(|| unreadable("no JSON result"))?;
  if result.get("is_error").and_then(Value::as_bool) == Some(true) {
    return Err(unreadable("the result is an error"));
  }
  let structured = result
    .get("structured_output")
    .cloned()
    .ok_or_else(|| unreadable("no structured output"))?;
  let answer: Answer = serde_json::from_value(structured).map_err(|error| unreadable(&error.to_string()))?;
  let reason = answer.reason.trim().to_owned();
  if reason.is_empty() {
    return Err(unreadable("an empty reason"));
  }
  Ok(match answer.verdict {
    Verdict::Approve => ReviewVerdict::Approve { reason },
    Verdict::Ask => ReviewVerdict::AskPerson { reason },
  })
}

/// The connection auto mode reviews with: the project's Claude Code CLI connection, the first by name
/// when it has several; `None` when it has none, and auto mode is unavailable.
pub fn reviewer_connection(connections: &[super::Connection]) -> Option<&super::Connection> {
  connections
    .iter()
    .filter(|connection| {
      matches!(
        connection.kind(),
        super::ConnectionKind::Agent(super::AgentProvider::ClaudeCodeCli)
      )
    })
    .filter(|connection| connection.executable().is_some())
    .min_by(|left, right| left.name().as_str().cmp(right.name().as_str()))
}
