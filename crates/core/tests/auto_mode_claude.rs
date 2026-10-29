//! Auto mode's Claude Code reviewer: the command it runs and how it reads the structured verdict.
//! `cargo test -p orchester-core --test auto_mode_claude auto_mode`.

use std::{cell::RefCell, path::Path};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  connection::reviewer::{ClaudeReviewer, ReviewFolder, parse_review},
  permission::{NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::review::{
    RequestReviewer, ReviewError, ReviewInput, ReviewVerdict, ReviewedRequest, ReviewedStep, ReviewedTask,
  },
};

fn model() -> ModelId {
  ModelId::new("claude-haiku-4-5-20251001").expect("model")
}

fn input() -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "cargo build".to_owned(),
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
    &[],
  )
  .expect("within the size of a review")
}

/// A CLI that answers every command with `output`, or fails with `error`, and keeps the command.
struct Cli {
  answer: Result<CommandOutput, CommandError>,
  seen: RefCell<Option<Command>>,
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
    seen: RefCell::new(None),
  }
}

fn printed(stdout: &str) -> Result<CommandOutput, CommandError> {
  Ok(CommandOutput {
    code: Some(0),
    stdout: stdout.to_owned(),
    stderr: String::new(),
  })
}

/// The folder a CLI reviewer runs in.
fn folder() -> ReviewFolder {
  ReviewFolder {
    work: "/tmp/review/work".into(),
    instructions: "/tmp/review/instructions.md".into(),
    schema: "/tmp/review/schema.json".into(),
  }
}

#[test]
fn auto_mode_claude_reviewer_reads_the_structured_verdict() {
  let worktree = Path::new("/tmp/review/work");
  let answered = r#"{"type":"result","subtype":"success","is_error":false,"result":"","structured_output":{"verdict":"approve","reason":"It regenerates the fixtures."}}"#;
  let runner = cli(printed(answered));
  let reviewer = ClaudeReviewer::new(
    Path::new("/bin/claude"),
    (model(), Some("high".to_owned())),
    (folder(), vec![("PATH".to_owned(), "/bin".to_owned())]),
    runner,
  );
  assert_eq!(
    reviewer.review(&input()).verdict,
    Ok(ReviewVerdict::Approve {
      reason: "It regenerates the fixtures.".to_owned()
    })
  );
  let command = reviewer.command(&input());
  assert_eq!(command.program(), Path::new("/bin/claude"));
  assert_eq!(command.current_dir(), Some(worktree));
  assert!(command.stdin().is_some_and(|prompt| prompt.contains("cargo build")));
  assert!(command.args().iter().any(|arg| arg == "/tmp/review/instructions.md"));
  assert!(
    command
      .args()
      .windows(2)
      .any(|pair| pair[0] == "--effort" && pair[1] == "high"),
    "the chosen effort"
  );
  assert!(command.secret_env().is_empty());

  let timed_out = ClaudeReviewer::new(
    Path::new("/bin/claude"),
    (model(), None),
    (folder(), Vec::new()),
    cli(Err(CommandError::TimedOut)),
  );
  assert_eq!(timed_out.review(&input()).verdict, Err(ReviewError::TimedOut));
  for stdout in [
    "Sure, approve it.",
    r#"{"type":"result","is_error":true,"structured_output":{"verdict":"approve","reason":"x"}}"#,
    r#"{"type":"result","is_error":false}"#,
    r#"{"type":"assistant","is_error":false,"structured_output":{"verdict":"approve","reason":"x"}}"#,
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
