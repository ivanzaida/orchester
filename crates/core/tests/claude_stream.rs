//! Claude Code `-p --output-format stream-json --verbose` output. `not-signed-in-clean-env` is real
//! output of Claude Code 2.1.89, started with the cleaned agent environment and the read-only
//! arguments (only the working directory is replaced). The CLI was not signed in, so no model turn
//! was ever observed: the `synthetic-*` fixtures are written by hand after the event shapes the
//! probe recorded (`.tmp/cli-probe/REPORT.md`) and are not captured output.

use orchester_core::{
  connection::stream::{
    AgentEvent, ClaudeStream, CommandStatus, FileChange, FileChangeKind, InvocationOutcome, InvocationTracker,
    OutputStream,
  },
  run::{StepOutcome, StepResult, TokenUsage},
};

fn replay(fixture: &str) -> Vec<AgentEvent> {
  let mut stream = ClaudeStream::default();
  fixture
    .lines()
    .flat_map(|line| stream.parse(OutputStream::Stdout, line))
    .collect()
}

fn tracked(events: &[AgentEvent]) -> InvocationTracker {
  let mut tracker = InvocationTracker::default();
  for event in events {
    tracker.observe(event);
  }
  tracker
}

#[test]
fn claude_stream_reads_a_cli_that_is_not_signed_in_as_a_typed_failure() {
  let events = replay(include_str!("fixtures/claude/not-signed-in-clean-env.jsonl"));
  assert_eq!(
    events,
    [
      AgentEvent::SessionStarted {
        session: "0199a0a0-0000-7000-8000-000000000001".into()
      },
      AgentEvent::TurnStarted,
      AgentEvent::NotSignedIn {
        message: "Not logged in · Please run /login".into()
      },
    ]
  );
  let tracker = tracked(&events);
  assert_eq!(
    tracker.outcome(Some(1)),
    InvocationOutcome::NotSignedIn {
      message: "Not logged in · Please run /login".into()
    }
  );
  assert_eq!(tracker.usage(), TokenUsage::default());
}

#[test]
fn claude_stream_reads_tool_calls_file_changes_and_the_final_message() {
  let events = replay(include_str!("fixtures/claude/synthetic-tool-use.jsonl"));
  let worktree = "/repo/.orchester/worktrees/run";
  let expected_start = [
    AgentEvent::SessionStarted {
      session: "5f0c2a1e-7b3d-4c8e-9a61-2d4b8e0f1a37".into(),
    },
    AgentEvent::TurnStarted,
    AgentEvent::Reasoning {
      text: "I should look at the README first.".into(),
    },
  ];
  assert_eq!(events[..3], expected_start);
  assert!(events.contains(&AgentEvent::Message {
    text: "Checking the readme.".into()
  }));
  assert!(events.contains(&AgentEvent::CommandStarted {
    item: "toolu_01".into(),
    command: "git status --short".into()
  }));
  assert!(events.contains(&AgentEvent::CommandFinished {
    item: "toolu_01".into(),
    command: "git status --short".into(),
    output: "?? notes.txt".into(),
    exit_code: None,
    status: CommandStatus::Completed,
  }));
  assert!(events.contains(&AgentEvent::CommandFinished {
    item: "toolu_02".into(),
    command: format!("Read {worktree}/README.md"),
    output: "     1\thi".into(),
    exit_code: None,
    status: CommandStatus::Completed,
  }));
  assert!(events.contains(&AgentEvent::FileChanged {
    item: "toolu_03".into(),
    changes: vec![FileChange {
      path: format!("{worktree}/README.md"),
      kind: FileChangeKind::Modified
    }],
  }));
  assert!(events.contains(&AgentEvent::FileChanged {
    item: "toolu_04".into(),
    changes: vec![FileChange {
      path: format!("{worktree}/NOTES.md"),
      kind: FileChangeKind::Added
    }],
  }));
  // Edits are shown as file changes, not as commands.
  assert!(!events.iter().any(|event| matches!(
    event,
    AgentEvent::CommandStarted { item, .. } if item == "toolu_03" || item == "toolu_04"
  )));
  // The result repeats the last message; it is not shown twice.
  let messages = events
    .iter()
    .filter(|event| matches!(event, AgentEvent::Message { .. }))
    .count();
  assert_eq!(messages, 2);
  assert!(
    !events.iter().any(|event| matches!(event, AgentEvent::Raw { .. })),
    "{events:#?}"
  );
  let tracker = tracked(&events);
  assert_eq!(tracker.session(), Some("5f0c2a1e-7b3d-4c8e-9a61-2d4b8e0f1a37"));
  assert_eq!(
    tracker.last_message(),
    Some(r#"{"outcome":"done","summary":"Appended hello."}"#)
  );
  assert_eq!(tracker.outcome(Some(0)), InvocationOutcome::Completed);
}

#[test]
fn claude_stream_counts_each_message_once_and_cache_reads_as_not_counted() {
  let events = replay(include_str!("fixtures/claude/synthetic-tool-use.jsonl"));
  let turns = events
    .iter()
    .filter(|event| matches!(event, AgentEvent::TurnStarted))
    .count();
  assert_eq!(turns, 3, "one turn per assistant message, however many lines it takes");
  let usage = tracked(&events).usage();
  // Three messages: input 12 + 8 + 5, cache writes 5,000 + 300 + 0, cache reads 1,000 + 6,000 +
  // 6,300, output 40 + 60 + 30. The result's total adds nothing beyond them.
  assert_eq!(
    usage,
    TokenUsage {
      input: 25 + 5_300 + 13_300,
      cached_input: 13_300,
      output: 130,
    }
  );
  assert_eq!(usage.counted(), 25 + 5_300 + 130);
}

#[test]
fn claude_stream_counts_what_only_the_result_reports() {
  let mut stream = ClaudeStream::default();
  let result = r#"{"type":"result","subtype":"success","is_error":false,"result":"Done.","usage":{"input_tokens":10,"output_tokens":4,"cache_creation_input_tokens":2,"cache_read_input_tokens":100},"permission_denials":[]}"#;
  let events = stream.parse(OutputStream::Stdout, result);
  assert_eq!(
    events,
    [
      AgentEvent::TurnCompleted {
        usage: TokenUsage {
          input: 112,
          cached_input: 100,
          output: 4
        }
      },
      AgentEvent::Message { text: "Done.".into() },
      AgentEvent::TurnFailed {
        message: "Claude Code ended its turn without the step result: a JSON object with outcome and summary".into()
      },
    ]
  );
}

fn result_line(text: &str, structured: Option<&str>) -> String {
  let structured = structured.map_or_else(String::new, |output| format!(r#","structured_output":{output}"#));
  format!(
    r#"{{"type":"result","subtype":"success","is_error":false,"result":{},"usage":{{"input_tokens":0,"output_tokens":0}},"permission_denials":[]{structured}}}"#,
    serde_json::to_string(text).expect("JSON")
  )
}

/// The events of `line` and the outcome and summary of the step result the last message holds.
fn last_message(line: &str) -> (Vec<AgentEvent>, Option<(StepOutcome, String)>) {
  let mut stream = ClaudeStream::default();
  let events = stream.parse(OutputStream::Stdout, line);
  let result = tracked(&events)
    .last_message()
    .and_then(StepResult::parse)
    .map(|result| (result.outcome, result.summary));
  (events, result)
}

#[test]
fn claude_stream_takes_the_structured_output_as_the_step_result() {
  let question = r#"{"outcome":"question","summary":"Which database?"}"#;
  let (events, result) = last_message(&result_line("I need a decision first.", Some(question)));
  assert_eq!(result, Some((StepOutcome::Question, "Which database?".to_owned())));
  assert!(
    !events
      .iter()
      .any(|event| matches!(event, AgentEvent::TurnFailed { .. }))
  );
}

#[test]
fn claude_stream_takes_the_result_that_closes_prose_or_fails_the_turn() {
  let failed = r#"{"outcome":"failed","summary":"The build breaks."}"#;
  for text in [
    format!("The build breaks at step 3.\n\n{failed}"),
    format!("The build breaks.\n\n```json\n{failed}\n```\n"),
    "Report:\n{\n  \"outcome\": \"failed\",\n  \"summary\": \"The build breaks.\"\n}".to_owned(),
  ] {
    let (_, result) = last_message(&result_line(&text, None));
    assert_eq!(
      result,
      Some((StepOutcome::Failed, "The build breaks.".to_owned())),
      "{text}"
    );
  }
  for text in [
    "All done, it works.".to_owned(),
    format!("{failed}\nAnd one more thing."),
    r#"Result: {"outcome":"done","summary":"x"}"#.to_owned(),
    r#"{"outcome":"maybe","summary":"x"}"#.to_owned(),
  ] {
    let (events, _) = last_message(&result_line(&text, None));
    assert!(
      events
        .iter()
        .any(|event| matches!(event, AgentEvent::TurnFailed { .. })),
      "{text} must not pass"
    );
    assert!(!tracked(&events).outcome(Some(0)).is_success(), "{text}");
  }
}

#[test]
fn claude_stream_reports_permission_denials_and_goes_on() {
  let events = replay(include_str!("fixtures/claude/synthetic-permission-denial.jsonl"));
  assert!(events.contains(&AgentEvent::CommandFinished {
    item: "toolu_11".into(),
    command: "curl https://example.com".into(),
    output: "Claude requested permissions to use Bash, but you haven't granted it yet.".into(),
    exit_code: None,
    status: CommandStatus::Failed,
  }));
  assert!(events.contains(&AgentEvent::PermissionDenied {
    item: "toolu_11".into(),
    command: "curl https://example.com".into(),
  }));
  let tracker = tracked(&events);
  assert_eq!(tracker.outcome(Some(0)), InvocationOutcome::Completed);
  assert_eq!(tracker.usage().counted(), 25 + 45);
  // The result that closes the prose becomes the last message.
  let result = tracker
    .last_message()
    .and_then(StepResult::parse)
    .expect("a step result");
  assert_eq!(
    (result.outcome, result.summary.as_str()),
    (StepOutcome::Done, "Went on without the page.")
  );
}

#[test]
fn claude_stream_reads_an_error_result_as_a_failed_turn() {
  let mut stream = ClaudeStream::default();
  let result = r#"{"type":"result","subtype":"error_max_turns","is_error":true,"usage":{"input_tokens":0,"output_tokens":0},"permission_denials":[]}"#;
  let events = stream.parse(OutputStream::Stdout, result);
  assert_eq!(
    events,
    [AgentEvent::TurnFailed {
      message: "error_max_turns".into()
    }]
  );
  assert_eq!(
    tracked(&events).outcome(Some(1)),
    InvocationOutcome::TurnFailed {
      message: "error_max_turns".into()
    }
  );
}

#[test]
fn claude_stream_keeps_what_it_does_not_understand() {
  let mut stream = ClaudeStream::default();
  for (output, line) in [
    (OutputStream::Stdout, "not json"),
    (OutputStream::Stdout, r#"{"type":"system","subtype":"hook_started"}"#),
    (OutputStream::Stdout, r#"{"type":"stream_event"}"#),
    (
      OutputStream::Stderr,
      "Error: When using --print, --output-format=stream-json requires --verbose",
    ),
  ] {
    assert_eq!(
      stream.parse(output, line),
      [AgentEvent::Raw {
        stream: output,
        line: line.into()
      }]
    );
  }
}

#[test]
fn claude_stream_never_counts_a_message_twice_when_its_usage_repeats_or_shrinks() {
  let mut stream = ClaudeStream::default();
  let line = |usage: &str| {
    format!(
      r#"{{"type":"assistant","message":{{"id":"msg_1","content":[{{"type":"text","text":"x"}}],"usage":{usage}}}}}"#
    )
  };
  let mut events = Vec::new();
  for usage in [
    r#"{"input_tokens":10,"output_tokens":5}"#,
    r#"{"input_tokens":0,"output_tokens":0}"#,
    r#"{"input_tokens":10,"output_tokens":8}"#,
  ] {
    events.extend(stream.parse(OutputStream::Stdout, &line(usage)));
  }
  assert_eq!(
    tracked(&events).usage(),
    TokenUsage {
      input: 10,
      cached_input: 0,
      output: 8
    }
  );
}
