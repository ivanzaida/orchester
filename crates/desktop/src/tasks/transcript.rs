//! The Activity tab's transcript (02.I `SfXU1`): a run's stored agent events turned into the
//! entries a person reads. A command's start and end become one entry (a tool step's command tool
//! too), a permission denial marks the command it denied, each file of a file change is an entry of
//! its own (it opens the file in the Changes tab), consecutive unparsed lines become one collapsed
//! entry, a call of an MCP tool that runs without asking marks its command (Claude Code) or is an entry
//! of its own (Codex, which reports no command for it), and a failed turn
//! that repeats the error before it is not shown twice. Session and turn starts carry no text of their own; the
//! invocation panel shows the session. Nothing else is left out.

use std::sync::Arc;

use orchester_core::{
  connection::{
    mcp::McpServerProblem,
    stream::{AgentEvent, CommandStatus, FileChange, OutputStream, RunnerProblem, ToolCommandResult},
  },
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
  FileChange(FileChange),
  Usage(TokenUsage),
  Error(Arc<str>),
  /// The agent CLI is not signed in; what it said.
  NotSignedIn(Arc<str>),
  Problem(RunnerProblem),
  /// The agent works without an MCP server: its name when known, and why.
  McpServer(Option<Arc<str>>, McpServerProblem),
  /// Unparsed lines, in order.
  Raw(Arc<Vec<(OutputStream, Arc<str>)>>),
  /// A tool step's command tool, from its start to its end.
  ToolCommand(ToolCommandEntry),
  /// A call of an MCP tool that ran without asking, when no command shows it.
  McpTool(McpToolEntry),
}

/// Why nobody was asked about a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandMark {
  /// The step asks before MCP actions, and `server` lists the tool among those that run without asking.
  ExemptTool { server: Arc<str> },
}

/// An MCP tool call that ran without asking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpToolEntry {
  pub tool: Arc<str>,
  pub mark: CommandMark,
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
  /// The CLI's id of the call.
  pub item: Arc<str>,
  pub command: Arc<str>,
  pub output: Arc<str>,
  /// `None` while the command runs.
  pub status: Option<CommandStatus>,
  pub exit_code: Option<i64>,
  /// Milliseconds from start to end, when both were seen.
  pub duration: Option<i64>,
  /// Why nobody was asked about it, when it says.
  pub mark: Option<CommandMark>,
}

/// One entry of the transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
  /// Sequence of the entry's first event: stable while the transcript grows.
  pub sequence: u64,
  /// Position among the entries of that event: a file change lists several files.
  pub part: usize,
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
      EntryKind::Error(_) | EntryKind::NotSignedIn(_) | EntryKind::Problem(_) | EntryKind::McpServer(..) => true,
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
      part: 0,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind,
    };
    match &stored.event {
      AgentEvent::SessionStarted { .. } | AgentEvent::TurnStarted => {}
      AgentEvent::Message { text } => entries.push(entry(EntryKind::Message(text.as_str().into()))),
      AgentEvent::Reasoning { text } => entries.push(entry(EntryKind::Reasoning(text.as_str().into()))),
      AgentEvent::CommandStarted { item, command } => entries.push(entry(EntryKind::Command(CommandEntry {
        item: item.as_str().into(),
        command: command.as_str().into(),
        output: "".into(),
        status: None,
        exit_code: None,
        duration: None,
        mark: None,
      }))),
      AgentEvent::CommandFinished { .. } => finish_command(&mut entries, stored),
      AgentEvent::FileChanged { changes, .. } => {
        entries.extend(changes.iter().enumerate().map(|(part, change)| Entry {
          part,
          ..entry(EntryKind::FileChange(change.clone()))
        }));
      }
      AgentEvent::TurnCompleted { usage } => entries.push(entry(EntryKind::Usage(*usage))),
      AgentEvent::Error { message } => entries.push(entry(EntryKind::Error(message.as_str().into()))),
      AgentEvent::TurnFailed { message } => {
        let repeated = matches!(entries.last(), Some(Entry { kind: EntryKind::Error(previous), invocation, .. })
          if **previous == **message && *invocation == stored.invocation);
        if !repeated {
          entries.push(entry(EntryKind::Error(message.as_str().into())));
        }
      }
      AgentEvent::PermissionDenied { item, command } => deny_command(&mut entries, stored, (item, command)),
      AgentEvent::NotSignedIn { message } => entries.push(entry(EntryKind::NotSignedIn(message.as_str().into()))),
      AgentEvent::RunnerProblem { problem } => entries.push(entry(EntryKind::Problem(problem.clone()))),
      AgentEvent::McpServer { server, problem } => entries.push(entry(EntryKind::McpServer(
        server.as_deref().map(Arc::from),
        problem.clone(),
      ))),
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
      AgentEvent::McpToolNotAsked { .. } => not_asked(&mut entries, stored),
    }
  }
  entries
}

/// Marks the command of the call as not asked, or adds an entry for the call when no command shows it.
fn not_asked(entries: &mut Vec<Entry>, stored: &ActivityEntry) {
  let AgentEvent::McpToolNotAsked { item, server, tool } = &stored.event else {
    return;
  };
  let mark = CommandMark::ExemptTool {
    server: server.as_str().into(),
  };
  let shown = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
    EntryKind::Command(shown) if entry.invocation == stored.invocation && *shown.item == **item => Some(shown),
    _ => None,
  });
  match shown {
    Some(shown) => shown.mark = Some(mark),
    None => entries.push(Entry {
      sequence: stored.sequence,
      part: 0,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind: EntryKind::McpTool(McpToolEntry {
        tool: tool.as_str().into(),
        mark,
      }),
    }),
  }
}

/// Marks the latest entry of `command` in the same invocation as declined, or adds a declined one
/// when the command was not shown.
fn deny_command(entries: &mut Vec<Entry>, stored: &ActivityEntry, (item, command): (&str, &str)) {
  let shown = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
    EntryKind::Command(shown) if entry.invocation == stored.invocation && *shown.command == *command => Some(shown),
    _ => None,
  });
  match shown {
    Some(shown) => shown.status = Some(CommandStatus::Declined),
    None => entries.push(Entry {
      sequence: stored.sequence,
      part: 0,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind: EntryKind::Command(CommandEntry {
        item: item.into(),
        command: command.into(),
        output: "".into(),
        status: Some(CommandStatus::Declined),
        exit_code: None,
        duration: None,
        mark: None,
      }),
    }),
  }
}

/// Completes the entry its start created, or adds one when the start was not seen.
fn finish_command(entries: &mut Vec<Entry>, stored: &ActivityEntry) {
  let AgentEvent::CommandFinished {
    item,
    command,
    output,
    exit_code,
    status,
  } = &stored.event
  else {
    return;
  };
  let started = entries.iter_mut().rev().find(|entry| {
    entry.invocation == stored.invocation
      && matches!(&entry.kind, EntryKind::Command(started) if started.status.is_none() && *started.command == **command)
  });
  let finished = |duration, mark| {
    EntryKind::Command(CommandEntry {
      item: item.as_str().into(),
      command: command.as_str().into(),
      output: output.as_str().into(),
      status: Some(*status),
      exit_code: *exit_code,
      duration,
      mark,
    })
  };
  match started {
    Some(entry) => {
      let mark = match &entry.kind {
        EntryKind::Command(started) => started.mark.clone(),
        _ => None,
      };
      entry.kind = finished(Some(stored.at.unix_millis() - entry.at.unix_millis()), mark);
    }
    None => entries.push(Entry {
      sequence: stored.sequence,
      part: 0,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind: finished(None, None),
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
      part: 0,
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
    part: 0,
    execution: stored.execution,
    invocation: stored.invocation,
    at: stored.at,
    kind: EntryKind::Raw(Arc::new(vec![line])),
  });
}
