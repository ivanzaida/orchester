//! Why auto mode's reviewer or the story-point estimator gave no answer (ORC-587): the CLI's exit status with the error
//! it reported (Claude Code's error result, Codex's failed turn) or the last lines of its error output, kept as one
//! short line with the secret values of its invocation masked and nothing of its prompt.
//! `cargo test -p orchester-core --test reviewer_failure reviewer_failure`.

use std::path::Path;

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::reviewer::{ClaudeReviewer, CliEstimator, CodexReviewer, EstimatorCli, ReviewFolder},
  name::Name,
  permission::{NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::review::{
    DetailRedaction, RequestReviewer, Review, ReviewError, ReviewFailure, ReviewInput, ReviewedRequest, ReviewedStep,
    ReviewedTask,
  },
  task::{
    TaskDetails, TaskLimits,
    estimate::{EstimateInput, estimate_prompt},
  },
};

/// The value of a variable of the project the reviewer starts with.
const SECRET: &str = "orc587-project-token-value";
const SIGNED_OUT: &str = include_str!("fixtures/claude/not-signed-in-clean-env.jsonl");

fn model() -> ModelId {
  ModelId::new("claude-haiku-4-5-20251001").expect("model")
}

fn input() -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "cargo build --release".to_owned(),
      arguments: None,
      note: Some("Regenerate the fixtures".to_owned()),
      working_folder: Some(".orchester/worktrees/w1".to_owned()),
    },
    ReviewedStep {
      name: "Implement".to_owned(),
      repository: RepositoryAccess::Write,
      network: NetworkAccess::Off,
      writable_folders: Vec::new(),
    },
    ReviewedTask {
      title: "Refresh the fixtures".to_owned(),
      brief: "Run the generator and commit the fixtures it writes.".to_owned(),
      criteria: vec!["The fixtures match the generator".to_owned()],
    },
    &[],
  )
  .expect("within the size of a review")
}

/// A CLI that answers with `answer`; given `echo`, its standard input is put where `echo` says.
struct Cli {
  answer: CommandOutput,
  echo: fn(&mut CommandOutput, &str),
}

impl CommandRunner for Cli {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut answer = self.answer.clone();
    (self.echo)(&mut answer, command.stdin().unwrap_or_default());
    Ok(answer)
  }
}

fn exited(code: i32, stdout: &str, stderr: &str) -> CommandOutput {
  CommandOutput {
    code: Some(code),
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  }
}

fn folder() -> ReviewFolder {
  ReviewFolder {
    work: "/tmp/review/work".into(),
    instructions: "/tmp/review/instructions.md".into(),
    schema: "/tmp/review/schema.json".into(),
  }
}

/// The variables the reviewer starts with: the agent's own and the project's.
fn environment() -> Vec<(String, String)> {
  vec![
    ("PATH".to_owned(), "/bin".to_owned()),
    ("PROJECT_TOKEN".to_owned(), SECRET.to_owned()),
    ("FEATURE".to_owned(), "1".to_owned()),
  ]
}

/// What a Claude Code review that answered `answer` is kept with.
fn claude_detail(answer: CommandOutput, echo: fn(&mut CommandOutput, &str)) -> Option<String> {
  let reviewer = ClaudeReviewer::new(
    Path::new("/bin/claude"),
    (model(), None),
    (folder(), environment()),
    Cli { answer, echo },
  );
  match Review::of(reviewer.review(&input()).verdict, &model()) {
    Review::Failed {
      failure: ReviewFailure::CouldNotRun | ReviewFailure::Unreadable,
      detail,
      ..
    } => detail,
    other => panic!("{other:?}"),
  }
}

const fn as_is(_: &mut CommandOutput, _: &str) {}

#[test]
fn reviewer_failure_keeps_what_a_signed_out_claude_code_says() {
  assert_eq!(
    claude_detail(exited(1, SIGNED_OUT, ""), as_is).as_deref(),
    Some("the reviewer could not run: the CLI exited with code 1: Not logged in \u{b7} Please run /login")
  );
  assert_eq!(
    claude_detail(exited(0, SIGNED_OUT, ""), as_is).as_deref(),
    Some("the reviewer's answer could not be read: the result is an error: Not logged in \u{b7} Please run /login"),
    "an error result read from a CLI that exited successfully"
  );
}

#[test]
fn reviewer_failure_keeps_the_last_lines_of_the_error_output() {
  let stderr = "trace-a\ntrace-b\ntrace-c\n\ntrace-d\ntrace-e\ntrace-f\nfatal: the API is overloaded\n";
  assert_eq!(
    claude_detail(exited(3, "", stderr), as_is).as_deref(),
    Some(
      "the reviewer could not run: the CLI exited with code 3: trace-c trace-d trace-e trace-f fatal: the API is \
       overloaded"
    )
  );
  let detail = claude_detail(exited(1, "", &"x".repeat(2_000)), as_is).expect("a detail");
  assert!(detail.chars().count() <= 300, "{detail}");
}

#[test]
fn reviewer_failure_masks_secrets_and_leaves_out_the_prompt() {
  let stderr = format!("auth failed for {SECRET}\nvia https://me:{SECRET}x@proxy.example/ (code 1)\n");
  let detail = claude_detail(exited(2, "", &stderr), as_is).expect("a detail");
  assert!(!detail.contains(SECRET), "{detail}");
  assert!(!detail.contains("me:"), "{detail}");
  assert!(detail.contains("auth failed for <redacted>"), "{detail}");

  // A CLI whose error repeats its prompt: its own words stay, the prompt goes.
  let echoed = |answer: &mut CommandOutput, prompt: &str| {
    let said = format!("Credit balance is too low\n{prompt}");
    answer.stdout = serde_json::json!({"type": "result", "is_error": true, "result": said}).to_string();
    answer.stderr = prompt.to_owned();
  };
  let detail = claude_detail(exited(1, "", ""), echoed).expect("a detail");
  assert_eq!(
    detail,
    "the reviewer could not run: the CLI exited with code 1: Credit balance is too low"
  );
  let only_echo = |answer: &mut CommandOutput, prompt: &str| answer.stderr = prompt.to_owned();
  let detail = claude_detail(exited(1, "", ""), only_echo).expect("a detail");
  assert_eq!(detail, "the reviewer could not run: the CLI exited with code 1:");
}

#[test]
fn reviewer_failure_keeps_what_codex_reported() {
  let failed = concat!(
    r#"{"type":"thread.started","thread_id":"t"}"#,
    "\n",
    r#"{"type":"turn.started"}"#,
    "\n",
    r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit."}}"#,
  );
  for (code, expected) in [
    (
      1,
      "the reviewer could not run: the CLI exited with code 1: You've hit your usage limit.",
    ),
    (
      0,
      "the reviewer could not run: Codex reported turn.failed: You've hit your usage limit.",
    ),
  ] {
    let reviewer = CodexReviewer::new(
      Path::new("/bin/codex"),
      (model(), None),
      (folder(), environment()),
      Cli {
        answer: exited(code, failed, ""),
        echo: as_is,
      },
    );
    let error = reviewer.review(&input()).verdict.expect_err("the turn failed");
    assert_eq!(error.detail(), expected);
  }
}

#[test]
fn reviewer_failure_of_an_estimate_is_redacted_too() {
  let details = TaskDetails {
    title: Name::new("Refresh the fixtures").expect("title"),
    brief: "Run the generator and commit the fixtures it writes.".to_owned(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let input = EstimateInput::new(&details, &[]);
  let estimator = CliEstimator {
    cli: EstimatorCli::ClaudeCode,
    executable: "/bin/claude".into(),
    model: (model(), None),
    folder: folder(),
    environment: environment(),
  };
  let cli = Cli {
    answer: exited(1, "", ""),
    echo: |answer, prompt| answer.stderr = format!("{prompt}\nkey {SECRET} refused"),
  };
  let error = estimator.estimate(&input, &cli).answer.expect_err("no points");
  assert_eq!(error.failure(), ReviewFailure::CouldNotRun);
  assert_eq!(
    error.detail(),
    "the reviewer could not run: the CLI exited with code 1: key <redacted> refused"
  );
  assert!(!estimate_prompt(&input).contains("key <redacted>"));
}

#[test]
fn reviewer_failure_redaction_masks_given_secrets_and_keeps_a_timeout() {
  let redaction = DetailRedaction::new(vec![SECRET], "the whole prompt the model was given, word for word");
  let error = ReviewError::Unparseable(format!(
    "not one JSON object: {SECRET}\nthe whole prompt the model was given, word for word\nexpected value"
  ));
  assert_eq!(
    error.redacted(&redaction).detail(),
    "the reviewer's answer could not be read: not one JSON object: <redacted> expected value"
  );
  assert_eq!(ReviewError::TimedOut.redacted(&redaction), ReviewError::TimedOut);
}
