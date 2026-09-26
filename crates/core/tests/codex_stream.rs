//! Codex `exec --json` output, replayed from the CLI probe (`.tmp/cli-probe/out/codex`).

use orchester_core::{
  connection::stream::{
    AgentEvent, CommandStatus, FileChangeKind, InvocationOutcome, InvocationTracker, OutputStream, parse_codex_line,
  },
  run::TokenUsage,
};

fn replay(fixture: &str) -> Vec<AgentEvent> {
  fixture
    .lines()
    .map(|line| parse_codex_line(OutputStream::Stdout, line))
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
fn codex_stream_reads_a_completed_read_only_turn() {
  let events = replay(include_str!("fixtures/codex/c01-ok-readonly.jsonl"));
  assert_eq!(
    events,
    [
      AgentEvent::SessionStarted {
        session: "01a0d831-b556-7730-be9d-2491a9359250".into()
      },
      AgentEvent::TurnStarted,
      AgentEvent::Message { text: "OK".into() },
      AgentEvent::TurnCompleted {
        usage: TokenUsage {
          input: 12_166,
          cached_input: 2_432,
          output: 5,
        }
      },
    ]
  );
  let tracker = tracked(&events);
  assert_eq!(tracker.session(), Some("01a0d831-b556-7730-be9d-2491a9359250"));
  assert_eq!(tracker.usage().counted(), 12_166 - 2_432 + 5);
  assert_eq!(tracker.outcome(Some(0)), InvocationOutcome::Completed);
}

#[test]
fn codex_stream_reads_a_declined_command() {
  let events = replay(include_str!("fixtures/codex/c02-write-readonly.jsonl"));
  let declined = events.iter().find_map(|event| match event {
    AgentEvent::CommandFinished {
      item,
      exit_code,
      status,
      output,
      ..
    } => Some((item.as_str(), *exit_code, *status, output.as_str())),
    _ => None,
  });
  let (item, exit_code, status, output) = declined.expect("a finished command");
  assert_eq!((item, exit_code, status), ("item_1", Some(-1), CommandStatus::Declined));
  assert!(output.ends_with("rejected: blocked by policy"));
  assert!(matches!(&events[3], AgentEvent::CommandStarted { item, .. } if item == "item_1"));
  // A declined command does not fail the turn: the agent was told and finished.
  assert_eq!(tracked(&events).outcome(Some(0)), InvocationOutcome::Completed);
}

#[test]
fn codex_stream_reads_commands_with_exit_codes() {
  let events = replay(include_str!("fixtures/codex/c04-workspace-write-winsandbox.jsonl"));
  let finished: Vec<_> = events
    .iter()
    .filter_map(|event| match event {
      AgentEvent::CommandFinished { exit_code, status, .. } => Some((*exit_code, *status)),
      _ => None,
    })
    .collect();
  assert_eq!(
    finished,
    [
      (Some(0), CommandStatus::Completed),
      (Some(0), CommandStatus::Completed),
      (Some(1), CommandStatus::Failed),
    ]
  );
  assert!(events.iter().all(|event| !matches!(event, AgentEvent::Raw { .. })));
}

#[test]
fn codex_stream_reads_a_failed_turn_with_the_readable_message() {
  let events = replay(include_str!("fixtures/codex/c11-bad-model-via-cmd-shim.jsonl"));
  let expected = "The 'no-such-model-probe' model is not supported when using Codex with a ChatGPT account.";
  assert_eq!(
    events[2],
    AgentEvent::Error {
      message: expected.into()
    }
  );
  assert_eq!(
    events[3],
    AgentEvent::TurnFailed {
      message: expected.into()
    }
  );
  assert_eq!(
    tracked(&events).outcome(Some(1)),
    InvocationOutcome::TurnFailed {
      message: expected.into()
    }
  );
}

#[test]
fn codex_stream_reads_file_changes() {
  let line = r#"{"type":"item.completed","item":{"id":"item_4","type":"file_change","changes":[{"path":"README.md","kind":"update"},{"path":"new.txt","kind":"add"}],"status":"completed"}}"#;
  let AgentEvent::FileChanged { item, changes } = parse_codex_line(OutputStream::Stdout, line) else {
    panic!("a file change");
  };
  assert_eq!(item, "item_4");
  let kinds: Vec<_> = changes
    .iter()
    .map(|change| (change.path.as_str(), change.kind))
    .collect();
  assert_eq!(
    kinds,
    [
      ("README.md", FileChangeKind::Modified),
      ("new.txt", FileChangeKind::Added)
    ]
  );
}

#[test]
fn codex_stream_keeps_unknown_lines_and_stderr_as_raw() {
  for (stream, line) in [
    (OutputStream::Stdout, "not json"),
    (
      OutputStream::Stdout,
      r#"{"type":"item.updated","item":{"id":"x","type":"todo_list"}}"#,
    ),
    (
      OutputStream::Stdout,
      r#"{"type":"item.completed","item":{"id":"x","type":"web_search"}}"#,
    ),
    (OutputStream::Stderr, r#"{"type":"turn.started"}"#),
    (
      OutputStream::Stderr,
      "2026-09-25T10:53:21Z ERROR codex_core::tools::router: rejected",
    ),
  ] {
    assert_eq!(
      parse_codex_line(stream, line),
      AgentEvent::Raw {
        stream,
        line: line.into()
      }
    );
  }
}

#[test]
fn codex_stream_outcome_without_a_completed_turn_or_with_a_failure_code() {
  let session = [AgentEvent::SessionStarted { session: "s".into() }];
  assert_eq!(tracked(&session).outcome(Some(0)), InvocationOutcome::NoResult);
  assert_eq!(
    tracked(&session).outcome(Some(2)),
    InvocationOutcome::ExitFailed { code: Some(2) }
  );
  assert_eq!(
    tracked(&session).outcome(None),
    InvocationOutcome::ExitFailed { code: None }
  );
}
