//! Auto mode's review of the requests the rules leave to a person: hard rules first, which no reviewer
//! overrides; the reviewer's approval or question with its reason; a failure, timeout or unreadable answer
//! asking the person; and the Claude Code reviewer's arguments, prompt and answer.
//! `cargo test -p orchester-core --test auto_mode auto_mode`.

use std::{cell::Cell, path::Path};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::reviewer::{ClaudeReviewer, REVIEW_SCHEMA, parse_review, review_prompt, reviewer_args},
  id::{InvocationId, RunId, StepExecutionId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess, category::CommandScope},
  resource::ModelId,
  run::{
    ApprovalSubject, PendingToolApproval,
    review::{
      RequestReviewer, Review, ReviewError, ReviewFailure, ReviewHold, ReviewInput, ReviewVerdict, ReviewedRequest,
      ReviewedStep, ReviewedTask, review_hold, review_request,
    },
  },
  time::Timestamp,
};
use tempfile::TempDir;

/// A worktree in a home folder, to classify commands in.
struct Fixture {
  _root: TempDir,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("kontur");
    let temp = root.path().join("temp");
    for folder in [&worktree, &temp] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&temp));
    Self { _root: root, scope }
  }

  fn command(&self, command: &str) -> ApprovalSubject {
    let approval = PendingToolApproval {
      id: "invocation:1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "command".to_owned(),
      action: command.to_owned(),
      target: None,
      requested_at: Timestamp::now(),
      native: None,
    };
    ApprovalSubject::of(&approval, &self.scope)
  }
}

fn permissions(repository: RepositoryAccess, network: NetworkAccess) -> AgentPermissions {
  AgentPermissions::new(repository, CommandPermission::AskBefore, network)
}

fn writing() -> AgentPermissions {
  permissions(RepositoryAccess::Write, NetworkAccess::On)
}

fn model() -> ModelId {
  ModelId::new("claude-haiku-4-5-20251001").expect("model")
}

fn input(messages: &[String]) -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "./regen.sh".to_owned(),
      arguments: None,
      note: Some("Regenerate the fixtures".to_owned()),
      working_folder: Some(".orchester/worktrees/w1".to_owned()),
    },
    ReviewedStep {
      name: "Implement".to_owned(),
      repository: RepositoryAccess::Write,
      network: NetworkAccess::Off,
      writable_folders: vec!["../shared".to_owned()],
    },
    ReviewedTask {
      title: "Refresh the fixtures".to_owned(),
      brief: "Run the generator and commit the fixtures.".to_owned(),
      criteria: vec!["The fixtures match the generator".to_owned()],
    },
    messages,
  )
}

/// A reviewer that answers with `answer` and counts its reviews.
struct Fake {
  answer: Result<ReviewVerdict, ReviewError>,
  reviews: Cell<usize>,
  model: ModelId,
}

impl Fake {
  fn new(answer: Result<ReviewVerdict, ReviewError>) -> Self {
    Self {
      answer,
      reviews: Cell::new(0),
      model: model(),
    }
  }
}

impl RequestReviewer for Fake {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, _: &ReviewInput) -> Result<ReviewVerdict, ReviewError> {
    self.reviews.set(self.reviews.get() + 1);
    self.answer.clone()
  }
}

fn approve(reason: &str) -> Result<ReviewVerdict, ReviewError> {
  Ok(ReviewVerdict::Approve {
    reason: reason.to_owned(),
  })
}

#[test]
fn auto_mode_records_the_reviewers_verdict_and_fails_closed() {
  let fixture = Fixture::new();
  let subject = fixture.command("./regen.sh");
  let permissions = writing();
  let reviewed = |answer| review_request(Some(&Fake::new(answer)), (&subject, Some(&permissions)), &input(&[]));
  let approved = reviewed(approve(" It regenerates the task's fixtures. "));
  assert_eq!(
    approved,
    Review::Approved {
      reason: "It regenerates the task's fixtures.".to_owned(),
      model: "claude-haiku-4-5-20251001".to_owned()
    }
  );
  assert!(approved.approves());
  let asked = reviewed(Ok(ReviewVerdict::AskPerson {
    reason: "It is not needed for the task.".to_owned(),
  }));
  assert_eq!(asked.reason(), Some("It is not needed for the task."));
  assert!(!asked.approves());
  for (error, failure) in [
    (ReviewError::TimedOut, ReviewFailure::TimedOut),
    (ReviewError::Unparseable("prose".to_owned()), ReviewFailure::Unreadable),
    (
      ReviewError::CouldNotRun("not found".to_owned()),
      ReviewFailure::CouldNotRun,
    ),
  ] {
    let failed = reviewed(Err(error));
    assert!(
      matches!(failed, Review::Failed { failure: got, .. } if got == failure),
      "{failed:?}"
    );
    assert!(!failed.approves());
  }
  let without = review_request(None, (&subject, Some(&permissions)), &input(&[]));
  assert_eq!(
    without,
    Review::Failed {
      failure: ReviewFailure::NoReviewer,
      model: None
    }
  );
}

#[test]
fn auto_mode_hard_rules_win_over_any_reviewer() {
  let fixture = Fixture::new();
  let read_only = permissions(RepositoryAccess::ReadOnly, NetworkAccess::On);
  let offline = permissions(RepositoryAccess::Write, NetworkAccess::Off);
  let writing = writing();
  for (command, permissions, hold) in [
    ("cat ~/.ssh/id_rsa", Some(&writing), ReviewHold::Secrets),
    ("cargo test", Some(&read_only), ReviewHold::ReadOnlyStep),
    ("./regen.sh", Some(&read_only), ReviewHold::ReadOnlyStep),
    ("curl https://example.dev", Some(&offline), ReviewHold::NetworkOff),
    ("npm install left-pad", Some(&offline), ReviewHold::NetworkOff),
    ("cp src/a.rs ../../a.rs", Some(&writing), ReviewHold::OutsideFolders),
    ("./regen.sh", None, ReviewHold::PermissionsUnknown),
  ] {
    let subject = fixture.command(command);
    let reviewer = Fake::new(approve("Looks fine."));
    let review = review_request(Some(&reviewer), (&subject, permissions), &input(&[]));
    assert_eq!(review, Review::Held { hold }, "{command}");
    assert_eq!(reviewer.reviews.get(), 0, "{command}: no review");
  }
  for command in [
    "./regen.sh",
    "git push --force",
    "git status",
    "curl https://example.dev",
  ] {
    assert_eq!(
      review_hold(&fixture.command(command), Some(&writing)),
      None,
      "{command}"
    );
  }
}

#[test]
fn auto_mode_input_keeps_the_last_messages_within_their_size() {
  let messages: Vec<String> = (1..=5).map(|number| format!("message {number}")).collect();
  let kept = input(&messages);
  assert_eq!(kept.messages(), ["message 3", "message 4", "message 5"]);
  let long = input(&["x".repeat(5_000)]);
  assert!(long.messages()[0].ends_with("… (cut)"));
  assert!(long.messages()[0].chars().count() < 2_100);
}

#[test]
fn auto_mode_reviewer_arguments_take_no_tools_and_ask_for_the_schema() {
  assert_eq!(
    reviewer_args(&model()),
    [
      "-p",
      "--output-format",
      "json",
      "--model",
      "claude-haiku-4-5-20251001",
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
  );
  let schema: serde_json::Value = serde_json::from_str(REVIEW_SCHEMA).expect("valid JSON");
  assert_eq!(schema["required"], serde_json::json!(["verdict", "reason"]));
  let prompt = review_prompt(&input(&["I will regenerate the fixtures.".to_owned()]));
  for part in [
    "A shell command: ./regen.sh",
    "The agent's note: Regenerate the fixtures",
    "Implement: may write in its worktree; network off; writable folders besides the worktree: ../shared",
    "Refresh the fixtures",
    "- The fixtures match the generator",
    "I will regenerate the fixtures.",
    "answer \"ask\"",
  ] {
    assert!(prompt.contains(part), "{part} in {prompt}");
  }
}

/// A CLI that answers every command with `output`, or fails with `error`, and keeps the command.
struct Cli {
  answer: Result<CommandOutput, CommandError>,
  seen: std::cell::RefCell<Option<Command>>,
}

impl CommandRunner for Cli {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    *self.seen.borrow_mut() = Some(command.clone());
    self.answer.clone()
  }
}

fn cli(answer: Result<CommandOutput, CommandError>) -> Cli {
  Cli {
    answer,
    seen: std::cell::RefCell::new(None),
  }
}

fn printed(stdout: &str) -> Result<CommandOutput, CommandError> {
  Ok(CommandOutput {
    code: Some(0),
    stdout: stdout.to_owned(),
    stderr: String::new(),
  })
}

#[test]
fn auto_mode_claude_reviewer_reads_the_structured_verdict() {
  let worktree = Path::new("/work/kontur");
  let answered = r#"{"type":"result","subtype":"success","is_error":false,"result":"","structured_output":{"verdict":"approve","reason":"It regenerates the fixtures."}}"#;
  let runner = cli(printed(answered));
  let reviewer = ClaudeReviewer::new(
    Path::new("/bin/claude"),
    model(),
    (worktree, vec![("PATH".to_owned(), "/bin".to_owned())]),
    runner,
  );
  assert_eq!(
    reviewer.review(&input(&[])),
    Ok(ReviewVerdict::Approve {
      reason: "It regenerates the fixtures.".to_owned()
    })
  );
  let command = reviewer.command(&input(&[]));
  assert_eq!(command.program(), Path::new("/bin/claude"));
  assert_eq!(command.current_dir(), Some(worktree));
  assert!(command.stdin().is_some_and(|prompt| prompt.contains("./regen.sh")));
  assert!(command.secret_env().is_empty());

  let timed_out = ClaudeReviewer::new(
    Path::new("/bin/claude"),
    model(),
    (worktree, Vec::new()),
    cli(Err(CommandError::TimedOut)),
  );
  assert_eq!(timed_out.review(&input(&[])), Err(ReviewError::TimedOut));
  for stdout in [
    "Sure, approve it.",
    r#"{"type":"result","is_error":true,"structured_output":{"verdict":"approve","reason":"x"}}"#,
    r#"{"type":"result","is_error":false}"#,
    r#"{"type":"result","is_error":false,"structured_output":{"verdict":"maybe","reason":"x"}}"#,
    r#"{"type":"result","is_error":false,"structured_output":{"verdict":"approve","reason":"  "}}"#,
  ] {
    assert!(
      matches!(parse_review(stdout), Err(ReviewError::Unparseable(_))),
      "{stdout}"
    );
  }
  let asked = r#"{"type":"result","is_error":false,"structured_output":{"verdict":"ask","reason":"It deletes the home folder."}}"#;
  assert_eq!(
    parse_review(asked),
    Ok(ReviewVerdict::AskPerson {
      reason: "It deletes the home folder.".to_owned()
    })
  );
}
