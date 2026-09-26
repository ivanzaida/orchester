//! The Activity transcript built from stored agent events.

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, OutputStream},
  id::{InvocationId, StepExecutionId},
  run::{ActivityEntry, TokenUsage},
  time::Timestamp,
};
use orchester_desktop::tasks::transcript::{EntryKind, entries};

fn stored(events: Vec<AgentEvent>) -> Vec<ActivityEntry> {
  let (execution, invocation) = (StepExecutionId::generate(), InvocationId::generate());
  events
    .into_iter()
    .zip(1..)
    .map(|(event, sequence)| ActivityEntry {
      sequence,
      execution,
      invocation,
      at: Timestamp::from_unix_millis(i64::try_from(sequence).expect("small") * 1_000),
      event,
    })
    .collect()
}

#[test]
fn transcript_joins_a_command_start_and_end_and_groups_unparsed_lines() {
  let activity = stored(vec![
    AgentEvent::SessionStarted { session: "s".into() },
    AgentEvent::TurnStarted,
    AgentEvent::Message { text: "Working".into() },
    AgentEvent::CommandStarted {
      item: "item_1".into(),
      command: "pnpm test".into(),
    },
    AgentEvent::Raw {
      stream: OutputStream::Stderr,
      line: "warn one".into(),
    },
    AgentEvent::Raw {
      stream: OutputStream::Stdout,
      line: "noise".into(),
    },
    AgentEvent::CommandFinished {
      item: "item_1".into(),
      command: "pnpm test".into(),
      output: "FAIL".into(),
      exit_code: Some(1),
      status: CommandStatus::Failed,
    },
    AgentEvent::TurnCompleted {
      usage: TokenUsage {
        input: 10,
        cached_input: 4,
        output: 2,
      },
    },
  ]);
  let entries = entries(&activity);
  let kinds: Vec<&EntryKind> = entries.iter().map(|entry| &entry.kind).collect();
  assert_eq!(kinds.len(), 4, "{kinds:?}");
  assert_eq!(kinds[0], &EntryKind::Message("Working".into()));
  let EntryKind::Command(command) = kinds[1] else {
    panic!("a command, got {:?}", kinds[1]);
  };
  assert_eq!(
    (
      &*command.command,
      &*command.output,
      command.status,
      command.exit_code,
      command.duration
    ),
    ("pnpm test", "FAIL", Some(CommandStatus::Failed), Some(1), Some(3_000))
  );
  assert!(entries[1].is_failure());
  let EntryKind::Raw(lines) = kinds[2] else {
    panic!("raw lines, got {:?}", kinds[2]);
  };
  assert_eq!(lines.len(), 2);
  assert!(matches!(kinds[3], EntryKind::Usage(usage) if usage.counted() == 8));
}

#[test]
fn transcript_shows_a_repeated_turn_failure_once_and_keeps_a_running_command_open() {
  let activity = stored(vec![
    AgentEvent::CommandStarted {
      item: "item_1".into(),
      command: "cargo build".into(),
    },
    AgentEvent::Error {
      message: "bad model".into(),
    },
    AgentEvent::TurnFailed {
      message: "bad model".into(),
    },
  ]);
  let entries = entries(&activity);
  assert_eq!(entries.len(), 2);
  assert!(matches!(&entries[0].kind, EntryKind::Command(command) if command.status.is_none()));
  assert_eq!(entries[1].kind, EntryKind::Error("bad model".into()));
}

#[test]
fn transcript_marks_a_denied_command_and_shows_a_denial_without_a_command() {
  let activity = stored(vec![
    AgentEvent::CommandStarted {
      item: "toolu_1".into(),
      command: "curl https://example.com".into(),
    },
    AgentEvent::CommandFinished {
      item: "toolu_1".into(),
      command: "curl https://example.com".into(),
      output: "Claude requested permissions to use Bash, but you haven't granted it yet.".into(),
      exit_code: None,
      status: CommandStatus::Failed,
    },
    AgentEvent::PermissionDenied {
      item: "toolu_1".into(),
      command: "curl https://example.com".into(),
    },
    AgentEvent::PermissionDenied {
      item: "toolu_2".into(),
      command: "WebFetch https://example.com".into(),
    },
  ]);
  let entries = entries(&activity);
  let commands: Vec<(&str, Option<CommandStatus>)> = entries
    .iter()
    .filter_map(|entry| match &entry.kind {
      EntryKind::Command(command) => Some((&*command.command, command.status)),
      _ => None,
    })
    .collect();
  assert_eq!(
    commands,
    [
      ("curl https://example.com", Some(CommandStatus::Declined)),
      ("WebFetch https://example.com", Some(CommandStatus::Declined)),
    ]
  );
  assert!(entries.iter().all(|entry| entry.is_failure()));
}

#[test]
fn transcript_shows_a_cli_that_is_not_signed_in_as_a_failure() {
  let activity = stored(vec![
    AgentEvent::SessionStarted { session: "s".into() },
    AgentEvent::TurnStarted,
    AgentEvent::NotSignedIn {
      message: "Not logged in · Please run /login".into(),
    },
  ]);
  let entries = entries(&activity);
  assert_eq!(entries.len(), 1);
  assert_eq!(
    entries[0].kind,
    EntryKind::NotSignedIn("Not logged in · Please run /login".into())
  );
  assert!(entries[0].is_failure());
}
