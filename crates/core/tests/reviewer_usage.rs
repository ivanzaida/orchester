//! What auto mode's reviewer spent, read from each provider's answer: Claude Code's `stream-json` result
//! usage and its `rate_limit_event`s, the usage of Codex's `turn.completed`, and a native response's
//! usage; a review that ran but reported no readable usage is unknown, never zero, and one that never
//! started spent nothing. What it cost reaches the review's record and counts toward its step's budget.
//! The fixtures (`fixtures/reviewer/`) are shaped like Claude Code 2.1.286's `-p --output-format
//! stream-json --verbose` output and codex-cli 0.159.2's `exec --json` events (event names read from the
//! installed binary); their ids and figures are illustrative. `cargo test -p orchester-core --test
//! reviewer_usage reviewer_usage`.

use std::path::Path;

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::{
    AgentProvider,
    plan_limits::LimitState,
    reviewer::{ClaudeReviewer, CodexReviewer, ReviewFolder, native_review, read_claude_review, read_codex_review},
  },
  id::{InvocationId, RunId, StepExecutionId},
  native::{
    item::Item,
    model::{ModelResponse, StopReason},
  },
  permission::{NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::{
    Invocation, InvocationKind, InvocationTiming, TokenUsage, execution_tokens,
    review::{
      RequestReviewer, Review, ReviewCost, ReviewEnd, ReviewError, ReviewFailure, ReviewInput, ReviewRecord,
      ReviewSpend, ReviewVerdict, Reviewed, ReviewedRequest, ReviewedStep, ReviewedTask,
    },
  },
  time::Timestamp,
  tokens::TokensUsed,
};

const CLAUDE: &str = include_str!("fixtures/reviewer/claude-review.jsonl");
const CODEX: &str = include_str!("fixtures/reviewer/codex-review.jsonl");

fn usage(input: u64, cached_input: u64, output: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input,
    output,
  }
}

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("model")
}

fn input() -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "cargo build".to_owned(),
      arguments: None,
      note: None,
      working_folder: None,
    },
    ReviewedStep {
      name: "Implement".to_owned(),
      repository: RepositoryAccess::Write,
      network: NetworkAccess::On,
      writable_folders: Vec::new(),
    },
    ReviewedTask {
      title: "Build".to_owned(),
      brief: String::new(),
      criteria: Vec::new(),
    },
    &[],
  )
  .expect("within size")
}

fn folder() -> ReviewFolder {
  ReviewFolder {
    work: "/tmp/review/work".into(),
    instructions: "/tmp/review/instructions.md".into(),
    schema: "/tmp/review/schema.json".into(),
  }
}

/// A CLI that answers every command with `answer`.
struct Cli(Result<CommandOutput, CommandError>);

impl CommandRunner for Cli {
  fn run(&self, _: &Command) -> Result<CommandOutput, CommandError> {
    self.0.clone()
  }
}

fn exited(code: i32, stdout: &str) -> Result<CommandOutput, CommandError> {
  Ok(CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: String::new(),
  })
}

fn claude(answer: Result<CommandOutput, CommandError>) -> Reviewed {
  let reviewer = ClaudeReviewer::new(
    Path::new("/bin/claude"),
    (model("claude-haiku-4-5-20251001"), None),
    (folder(), Vec::new()),
    Cli(answer),
  );
  reviewer.review(&input())
}

#[test]
fn reviewer_usage_of_claude_code_is_its_results_usage_with_its_plan_limits() {
  let read = read_claude_review(CLAUDE);
  assert_eq!(
    read.verdict,
    Ok(ReviewVerdict::Approve {
      reason: "Building the project serves the task.".to_owned()
    })
  );
  // Cache reads are cached input; cache writes would count as input.
  assert_eq!(read.spend, ReviewSpend::Reported(usage(13_040, 11_200, 62)));
  assert_eq!(read.limits.len(), 1, "{:?}", read.limits);
  let window = &read.limits[0].windows[0];
  assert_eq!((window.minutes(), window.percent()), (300, Some(12.0)));
  assert_eq!(window.state(Timestamp::from_unix_millis(0)), LimitState::Normal);

  let without_usage = CLAUDE.replace(r#""usage":{"input_tokens":1840,"cache_creation_input_tokens":0,"cache_read_input_tokens":11200,"output_tokens":62,"server_tool_use""#, r#""spent":{"server_tool_use""#);
  assert_ne!(without_usage, CLAUDE);
  let read = read_claude_review(&without_usage);
  assert_eq!(
    read.spend,
    ReviewSpend::Unknown,
    "the assistant message's usage is not the review's"
  );
  assert!(read.verdict.is_ok());
}

#[test]
fn reviewer_usage_of_a_cli_is_read_whatever_its_exit_and_unknown_when_it_was_ended() {
  let failed = claude(exited(1, CLAUDE));
  assert!(matches!(failed.verdict, Err(ReviewError::CouldNotRun(_))), "{failed:?}");
  assert_eq!(failed.spend, ReviewSpend::Reported(usage(13_040, 11_200, 62)));
  let timed_out = claude(Err(CommandError::TimedOut));
  assert_eq!(
    (timed_out.verdict, timed_out.spend),
    (Err(ReviewError::TimedOut), ReviewSpend::Unknown)
  );
  let stopped = claude(Err(CommandError::Failed("the review is no longer wanted".to_owned())));
  assert_eq!(stopped.spend, ReviewSpend::Unknown);
  assert_eq!(claude(Err(CommandError::NotFound)).spend, ReviewSpend::Nothing);
  assert_eq!(claude(exited(0, "Sure, approve it.")).spend, ReviewSpend::Unknown);
}

#[test]
fn reviewer_usage_of_codex_is_its_turns_usage_without_plan_limits() {
  let read = read_codex_review(CODEX);
  assert_eq!(
    read.verdict,
    Ok(ReviewVerdict::AskPerson {
      reason: "It deletes files outside the worktree.".to_owned()
    })
  );
  // OpenAI-style: the input holds its cached part; reasoning is part of the output.
  assert_eq!(read.spend, ReviewSpend::Reported(usage(9_182, 6_912, 214)));
  assert!(read.limits.is_empty(), "codex exec reports no plan limits");

  let failed = "{\"type\":\"thread.started\"}\n{\"type\":\"turn.failed\",\"error\":{\"message\":\"x\"}}\n";
  assert_eq!(read_codex_review(failed).spend, ReviewSpend::Unknown);
  let reviewer = CodexReviewer::new(
    Path::new("/bin/codex"),
    (model("gpt-5.5-codex"), None),
    (folder(), Vec::new()),
    Cli(exited(0, CODEX)),
  );
  assert_eq!(
    reviewer.review(&input()).spend,
    ReviewSpend::Reported(usage(9_182, 6_912, 214))
  );
}

#[test]
fn reviewer_usage_of_a_native_review_is_its_responses_usage() {
  let response = ModelResponse {
    items: vec![Item::Assistant {
      text: r#"{"verdict":"approve","reason":"It builds."}"#.to_owned(),
    }],
    stop: StopReason::EndTurn,
    usage: usage(900, 300, 40),
    served_by: model("claude-haiku-4-5"),
  };
  let read = native_review(&response);
  assert!(read.verdict.is_ok());
  assert_eq!(read.spend, ReviewSpend::Reported(usage(900, 300, 40)));
  let cut = ModelResponse {
    stop: StopReason::MaxOutput,
    ..response
  };
  assert_eq!(native_review(&cut).spend, ReviewSpend::Reported(usage(900, 300, 40)));
}

#[test]
fn reviewer_usage_costs_only_a_review_that_asked_a_model() {
  let haiku = model("claude-haiku-4-5");
  let nothing = Reviewed::new(Err(ReviewError::CouldNotRun("x".to_owned())), ReviewSpend::Nothing);
  assert_eq!(ReviewCost::of(&nothing, &haiku), None);
  let unknown = Reviewed::new(Err(ReviewError::TimedOut), ReviewSpend::Unknown);
  let cost = ReviewCost::of(&unknown, &haiku).expect("a model was asked");
  assert_eq!((cost.model, cost.usage), (haiku.clone(), None));
  let reported = read_claude_review(CLAUDE);
  let cost = ReviewCost::of(&reported, &haiku).expect("a model was asked");
  assert_eq!(cost.usage, Some(usage(13_040, 11_200, 62)));
  assert_eq!(cost.limits, reported.limits);

  let approved = Review::Approved {
    reason: "x".to_owned(),
    model: haiku.as_str().to_owned(),
  };
  assert_eq!(ReviewEnd::of(&approved), Some(ReviewEnd::Approved));
  let failed = Review::Failed {
    failure: ReviewFailure::TimedOut,
    model: None,
  };
  assert_eq!(ReviewEnd::of(&failed), Some(ReviewEnd::Failed(ReviewFailure::TimedOut)));
}

#[test]
fn reviewer_usage_counts_toward_its_steps_budget_unless_unknown() {
  let (run, execution, other) = (
    RunId::generate(),
    StepExecutionId::generate(),
    StepExecutionId::generate(),
  );
  let agent = Invocation {
    id: InvocationId::generate(),
    run,
    execution,
    number: 1,
    kind: InvocationKind::Agent {
      model: model("gpt-5.5"),
    },
    permissions: None,
    session: None,
    started_at: Timestamp::from_unix_millis(0),
    ended_at: None,
    usage: usage(5_000, 1_000, 500),
    outcome: None,
    timing: InvocationTiming::default(),
  };
  let review = |execution, usage| ReviewRecord {
    run,
    execution,
    invocation: agent.id,
    approval: format!("{}:call_1", agent.id),
    provider: AgentProvider::ClaudeCodeCli,
    model: model("claude-haiku-4-5"),
    started_at: Timestamp::from_unix_millis(10),
    ended_at: Timestamp::from_unix_millis(20),
    usage,
    end: ReviewEnd::Approved,
  };
  let reviews = [
    review(execution, Some(usage(200, 50, 7))),
    review(execution, None),
    review(other, Some(usage(1_000, 0, 0))),
  ];
  assert_eq!(reviews[1].counted_tokens(), TokensUsed::ZERO, "unknown counts nothing");
  assert_eq!(
    execution_tokens(std::slice::from_ref(&agent), &reviews, execution),
    TokensUsed::new(4_500 + 157)
  );
  assert_eq!(execution_tokens(&[agent], &[], execution), TokensUsed::new(4_500));
}
