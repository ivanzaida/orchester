//! The Activity tab's transcript (02.I `SfXU1`): a run's stored agent events turned into the
//! entries a person reads. A command's start and end become one entry (a tool step's command tool
//! too), a permission denial marks
//! the command it denied, consecutive unparsed lines become one collapsed entry, and a failed turn
//! that repeats the error before it is not shown twice. Session and turn starts carry no text of their own; the
//! invocation panel shows the session. Nothing else is left out.

use std::sync::Arc;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, FileChange, OutputStream, RunnerProblem, ToolCommandResult},
  id::{InvocationId, StepExecutionId},
  run::{ActivityEntry, TokenUsage},
  time::Timestamp,
};

/// What one transcript entry shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKind {
  Message(Arc<str>),
  Reasoning(Arc<str>),
  Command(CommandEntry),
  FileChange(Arc<[FileChange]>),
  Usage(TokenUsage),
  Error(Arc<str>),
  /// The agent CLI is not signed in; what it said.
  NotSignedIn(Arc<str>),
  Problem(RunnerProblem),
  /// Unparsed lines, in order.
  Raw(Arc<Vec<(OutputStream, Arc<str>)>>),
  /// A tool step's command tool, from its start to its end.
  ToolCommand(ToolCommandEntry),
}

/// A command tool's program that a tool step ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCommandEntry {
  pub command: Arc<str>,
  /// `None` while the program runs.
  pub result: Option<ToolCommandResult>,
  pub exit_code: Option<i64>,
  /// The end of the output the tool keeps, after `omitted_bytes` that it did not keep.
  pub output: Arc<str>,
  pub omitted_bytes: u64,
  /// Milliseconds from start to end, when both were seen.
  pub duration: Option<i64>,
}

/// A command the agent ran, from its start to its end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandEntry {
  pub command: Arc<str>,
  pub output: Arc<str>,
  /// `None` while the command runs.
  pub status: Option<CommandStatus>,
  pub exit_code: Option<i64>,
  /// Milliseconds from start to end, when both were seen.
  pub duration: Option<i64>,
}

/// One entry of the transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
  /// Sequence of the entry's first event: stable while the transcript grows.
  pub sequence: u64,
  pub execution: StepExecutionId,
  pub invocation: InvocationId,
  pub at: Timestamp,
  pub kind: EntryKind,
}

impl Entry {
  /// A failed or declined command, an error or a runner problem: shown open, marked.
  pub fn is_failure(&self) -> bool {
    match &self.kind {
      EntryKind::Command(command) => matches!(command.status, Some(CommandStatus::Failed | CommandStatus::Declined)),
      EntryKind::ToolCommand(command) => matches!(
        command.result,
        Some(ToolCommandResult::Failed | ToolCommandResult::TimedOut)
      ),
      EntryKind::Error(_) | EntryKind::NotSignedIn(_) | EntryKind::Problem(_) => true,
      _ => false,
    }
  }
}

/// The entries of `activity`, in order.
pub fn entries(activity: &[ActivityEntry]) -> Vec<Entry> {
  let mut entries: Vec<Entry> = Vec::new();
  for stored in activity {
    let entry = |kind| Entry {
      sequence: stored.sequence,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind,
    };
    match &stored.event {
      AgentEvent::SessionStarted { .. } | AgentEvent::TurnStarted => {}
      AgentEvent::Message { text } => entries.push(entry(EntryKind::Message(text.as_str().into()))),
      AgentEvent::Reasoning { text } => entries.push(entry(EntryKind::Reasoning(text.as_str().into()))),
      AgentEvent::CommandStarted { command, .. } => entries.push(entry(EntryKind::Command(CommandEntry {
        command: command.as_str().into(),
        output: "".into(),
        status: None,
        exit_code: None,
        duration: None,
      }))),
      AgentEvent::CommandFinished { .. } => finish_command(&mut entries, stored),
      AgentEvent::FileChanged { changes, .. } => entries.push(entry(EntryKind::FileChange(changes.clone().into()))),
      AgentEvent::TurnCompleted { usage } => entries.push(entry(EntryKind::Usage(*usage))),
      AgentEvent::Error { message } => entries.push(entry(EntryKind::Error(message.as_str().into()))),
      AgentEvent::TurnFailed { message } => {
        let repeated = matches!(entries.last(), Some(Entry { kind: EntryKind::Error(previous), invocation, .. })
          if **previous == **message && *invocation == stored.invocation);
        if !repeated {
          entries.push(entry(EntryKind::Error(message.as_str().into())));
        }
      }
      AgentEvent::PermissionDenied { command, .. } => deny_command(&mut entries, stored, command),
      AgentEvent::NotSignedIn { message } => entries.push(entry(EntryKind::NotSignedIn(message.as_str().into()))),
      AgentEvent::RunnerProblem { problem } => entries.push(entry(EntryKind::Problem(problem.clone()))),
      AgentEvent::Raw { stream, line } => push_raw(&mut entries, stored, *stream, line),
      AgentEvent::ToolCommandStarted { command, .. } => entries.push(entry(EntryKind::ToolCommand(ToolCommandEntry {
        command: command.as_str().into(),
        result: None,
        exit_code: None,
        output: "".into(),
        omitted_bytes: 0,
        duration: None,
      }))),
      AgentEvent::ToolCommandFinished { .. } => finish_tool_command(&mut entries, stored),
    }
  }
  entries
}

/// Marks the latest entry of `command` in the same invocation as declined, or adds a declined one
/// when the command was not shown.
fn deny_command(entries: &mut Vec<Entry>, stored: &ActivityEntry, command: &str) {
  let shown = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
    EntryKind::Command(shown) if entry.invocation == stored.invocation && *shown.command == *command => Some(shown),
    _ => None,
  });
  match shown {
    Some(shown) => shown.status = Some(CommandStatus::Declined),
    None => entries.push(Entry {
      sequence: stored.sequence,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind: EntryKind::Command(CommandEntry {
        command: command.into(),
        output: "".into(),
        status: Some(CommandStatus::Declined),
        exit_code: None,
        duration: None,
      }),
    }),
  }
}

/// Completes the entry its start created, or adds one when the start was not seen.
fn finish_command(entries: &mut Vec<Entry>, stored: &ActivityEntry) {
  let AgentEvent::CommandFinished {
    command,
    output,
    exit_code,
    status,
    ..
  } = &stored.event
  else {
    return;
  };
  let started = entries.iter_mut().rev().find(|entry| {
    entry.invocation == stored.invocation
      && matches!(&entry.kind, EntryKind::Command(started) if started.status.is_none() && *started.command == **command)
  });
  let finished = |duration| {
    EntryKind::Command(CommandEntry {
      command: command.as_str().into(),
      output: output.as_str().into(),
      status: Some(*status),
      exit_code: *exit_code,
      duration,
    })
  };
  match started {
    Some(entry) => {
      entry.kind = finished(Some(stored.at.unix_millis() - entry.at.unix_millis()));
    }
    None => entries.push(Entry {
      sequence: stored.sequence,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind: finished(None),
    }),
  }
}

/// Completes the entry the command's start created, or adds one when the start was not seen.
fn finish_tool_command(entries: &mut Vec<Entry>, stored: &ActivityEntry) {
  let AgentEvent::ToolCommandFinished {
    command,
    result,
    exit_code,
    output,
    omitted_bytes,
    ..
  } = &stored.event
  else {
    return;
  };
  let finished = |duration| {
    EntryKind::ToolCommand(ToolCommandEntry {
      command: command.as_str().into(),
      result: Some(*result),
      exit_code: *exit_code,
      output: output.as_str().into(),
      omitted_bytes: *omitted_bytes,
      duration,
    })
  };
  let started = entries.iter_mut().rev().find(|entry| {
    entry.invocation == stored.invocation
      && matches!(&entry.kind, EntryKind::ToolCommand(started) if started.result.is_none())
  });
  match started {
    Some(entry) => entry.kind = finished(Some(stored.at.unix_millis() - entry.at.unix_millis())),
    None => entries.push(Entry {
      sequence: stored.sequence,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind: finished(None),
    }),
  }
}

fn push_raw(entries: &mut Vec<Entry>, stored: &ActivityEntry, stream: OutputStream, line: &str) {
  let line = (stream, Arc::<str>::from(line));
  if let Some(Entry {
    kind: EntryKind::Raw(lines),
    invocation,
    ..
  }) = entries.last_mut()
    && *invocation == stored.invocation
  {
    Arc::make_mut(lines).push(line);
    return;
  }
  entries.push(Entry {
    sequence: stored.sequence,
    execution: stored.execution,
    invocation: stored.invocation,
    at: stored.at,
    kind: EntryKind::Raw(Arc::new(vec![line])),
  });
}
