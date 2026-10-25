//! The Activity tab's transcript (02.I `SfXU1`): a run's stored agent events turned into the
//! entries a person reads. A command's start and end become one entry (a tool step's command tool
//! too), a permission denial marks the command it denied, each file of a file change is an entry of
//! its own (it opens the file in the Changes tab), consecutive unparsed lines become one collapsed
//! entry, a call of an MCP tool that runs without asking marks its command (Claude Code) or is an entry
//! of its own (Codex, which reports no command for it), and a failed turn that repeats the error before
//! it, or comes right after the request a closed permission prompt interrupted, is not shown twice. Session and turn
//! starts carry no text of their own; the invocation panel shows the session. A native agent's calls read as rows of
//! their tools ([`native`]), and so do a CLI's calls of Orchester's file tools ([`mcp`] for Codex's). Nothing else
//! is left out.

mod ended;
mod mcp;
mod native;
mod text;

use std::sync::Arc;

use orchester_core::{
  connection::{
    mcp::McpServerProblem,
    stream::{AgentEvent, CommandStatus, FileChange, NativeCall, OutputStream, RunnerProblem, ToolCommandResult},
  },
  id::{InvocationId, StepExecutionId},
  run::{ActivityEntry, TokenUsage, ToolDecision},
  time::Timestamp,
};

pub use self::{
  mcp::McpToolEntry,
  native::ApprovalEntry,
  text::{command_preview, in_worktree},
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
  /// A call of an MCP tool that no command shows: Codex reports none for it.
  McpTool(McpToolEntry),
  /// A tool listed to run without asking that asks after all: Claude Code could name it as a tool of
  /// `other`.
  ExemptionNotApplied {
    server: Arc<str>,
    tool: Arc<str>,
    other: Arc<str>,
  },
  /// A native agent's command a person or a rule approved.
  Approval(ApprovalEntry),
  /// A person stopped the agent's turn (*Stop turn*); its session goes on with their message.
  TurnStopped,
}

/// How a call of a tool of MCP server `server` was decided, as the decision marks of the command
/// categories design say it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandMark {
  /// The step asks before MCP actions, and `server` lists the tool among those that run without asking.
  ExemptTool { server: Arc<str> },
  /// `server` asked for approval in a step that does not ask before MCP actions: declined at once.
  Declined { server: Arc<str> },
  /// A person was asked, and the call ran.
  ApprovedByYou { server: Arc<str> },
  /// A person was asked, and the call did not run to its end.
  AskedYou { server: Arc<str> },
  /// Orchester recorded how the request was answered: by a person, a rule of the run, or auto-approval.
  Decided(DecidedMark),
}

impl CommandMark {
  /// A person or automation denied the request: the row stays open and shows what the agent was told.
  pub fn is_denial(&self) -> bool {
    matches!(self, Self::Decided(mark) if !mark.decision.decided_by.approves())
  }
}

/// A request's answer, as its mark names it: an MCP call by its server and tool, a command by its
/// category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecidedMark {
  pub server: Arc<str>,
  pub action: Arc<str>,
  /// Whether the request was about a command (not an MCP tool).
  pub command: bool,
  pub decision: ToolDecision,
  /// The transcript position and time of the answer.
  pub sequence: u64,
  pub at: Timestamp,
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
  /// What a native agent's call did, or what a CLI's call of one of Orchester's file tools asked; `None` for any
  /// other command.
  pub native: Option<NativeCall>,
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
      EntryKind::Command(command) => {
        matches!(command.status, Some(CommandStatus::Failed | CommandStatus::Declined))
          || command.mark.as_ref().is_some_and(CommandMark::is_denial)
      }
      EntryKind::ToolCommand(command) => matches!(
        command.result,
        Some(ToolCommandResult::Failed | ToolCommandResult::TimedOut)
      ),
      EntryKind::McpTool(call) => {
        matches!(call.mark, Some(CommandMark::Declined { .. }))
          || call.mark.as_ref().is_some_and(CommandMark::is_denial)
          || call.status == Some(CommandStatus::Failed)
      }
      EntryKind::Error(_) | EntryKind::NotSignedIn(_) | EntryKind::Problem(_) | EntryKind::McpServer(..) => true,
      _ => false,
    }
  }
}

/// What arrived before the entry it belongs to: answers recorded before the command they answer started
/// (Codex asks before it starts a command, a native agent before it runs one), and what a native call
/// did before its end made its entry.
#[derive(Default)]
struct Early {
  marks: Vec<(InvocationId, Arc<str>, CommandMark)>,
  told: native::Told,
}

impl Early {
  fn mark(&mut self, invocation: InvocationId, item: &str) -> Option<CommandMark> {
    let at = self
      .marks
      .iter()
      .position(|(early, early_item, _)| *early == invocation && **early_item == *item)?;
    Some(self.marks.remove(at).2)
  }
}

/// The entries of `activity`, in order.
pub fn entries(activity: &[ActivityEntry]) -> Vec<Entry> {
  let mut entries: Vec<Entry> = Vec::new();
  let mut early = Early::default();
  // The invocation whose request a closed prompt interrupted, for the one event right after it.
  let mut interrupted: Option<InvocationId> = None;
  for stored in activity {
    let explains = interrupted.take() == Some(stored.invocation);
    let entry = |kind| Entry {
      sequence: stored.sequence,
      part: 0,
      execution: stored.execution,
      invocation: stored.invocation,
      at: stored.at,
      kind,
    };
    match &stored.event {
      AgentEvent::SessionStarted { .. } | AgentEvent::TurnStarted | AgentEvent::PlanLimits { .. } => {}
      AgentEvent::Message { text } => entries.push(entry(EntryKind::Message(text.as_str().into()))),
      AgentEvent::Reasoning { text } => entries.push(entry(EntryKind::Reasoning(text.as_str().into()))),
      AgentEvent::CommandStarted { item, command } => {
        let mark = early.mark(stored.invocation, item);
        entries.push(entry(EntryKind::Command(CommandEntry {
          item: item.as_str().into(),
          command: command.as_str().into(),
          output: "".into(),
          status: None,
          exit_code: None,
          duration: None,
          mark,
          native: None,
        })));
      }
      AgentEvent::ToolDecided { .. } => decided(&mut entries, &mut early.marks, stored),
      AgentEvent::NativeCall { item, call } => early.told.note(&mut entries, stored, item, call),
      AgentEvent::CommandFinished { .. } => finish_command(&mut entries, &mut early, stored),
      AgentEvent::FileChanged { item, changes } => {
        if !native::shown_by_call(&entries, stored, item) {
          entries.extend(changes.iter().enumerate().map(|(part, change)| Entry {
            part,
            ..entry(EntryKind::FileChange(change.clone()))
          }));
        }
      }
      AgentEvent::TurnCompleted { usage } => entries.push(entry(EntryKind::Usage(*usage))),
      AgentEvent::Error { message } => entries.push(entry(EntryKind::Error(message.as_str().into()))),
      // The runner records the failed turn right after the request it interrupted, which says why.
      AgentEvent::TurnFailed { .. } if explains => {}
      AgentEvent::TurnFailed { message } => {
        if !repeats(entries.last(), stored, message) {
          entries.push(entry(EntryKind::Error(message.as_str().into())));
        }
      }
      AgentEvent::PermissionDenied { item, command } => {
        deny_command(&mut entries, &mut early, stored, (item, command));
      }
      AgentEvent::NotSignedIn { message } => entries.push(entry(EntryKind::NotSignedIn(message.as_str().into()))),
      AgentEvent::TurnStopped => entries.push(entry(EntryKind::TurnStopped)),
      AgentEvent::RunnerProblem { problem } => interrupted = push_problem(&mut entries, stored, problem),
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
      AgentEvent::McpToolNotAsked { .. } | AgentEvent::McpToolDeclined { .. } => mcp::not_asked(&mut entries, stored),
      AgentEvent::McpToolFinished { .. } => mcp::mcp_tool_finished(&mut entries, &mut early.told, stored),
      AgentEvent::McpExemptionNotApplied { server, tool, other } => entries.push(entry(exemption(server, tool, other))),
    }
  }
  native::approvals_apart(entries)
}

/// The entry of a tool of `server` listed to run without asking that asks after all, as a tool of `other`.
fn exemption(server: &str, tool: &str, other: &str) -> EntryKind {
  EntryKind::ExemptionNotApplied {
    server: server.into(),
    tool: tool.into(),
    other: other.into(),
  }
}

/// Adds the entry of runner problem `problem`; the invocation whose request it names as interrupted, if it
/// does.
fn push_problem(entries: &mut Vec<Entry>, stored: &ActivityEntry, problem: &RunnerProblem) -> Option<InvocationId> {
  entries.push(Entry {
    sequence: stored.sequence,
    part: 0,
    execution: stored.execution,
    invocation: stored.invocation,
    at: stored.at,
    kind: EntryKind::Problem(problem.clone()),
  });
  matches!(problem, RunnerProblem::DecisionInterrupted { .. }).then_some(stored.invocation)
}

/// Whether `last` is the error a failed turn of the same invocation repeats.
fn repeats(last: Option<&Entry>, stored: &ActivityEntry, message: &str) -> bool {
  matches!(last, Some(Entry { kind: EntryKind::Error(previous), invocation, .. })
    if **previous == *message && *invocation == stored.invocation)
}

/// Marks the entry of the call a request was about with its answer. A command whose start comes later
/// gets the mark then; an MCP call that no command shows gets an entry of its own, as for Codex.
fn decided(entries: &mut Vec<Entry>, early: &mut Vec<(InvocationId, Arc<str>, CommandMark)>, stored: &ActivityEntry) {
  let AgentEvent::ToolDecided {
    item,
    server,
    action,
    decision,
  } = &stored.event
  else {
    return;
  };
  let command = server == "command";
  let mark = CommandMark::Decided(DecidedMark {
    server: server.as_str().into(),
    action: action.as_str().into(),
    command,
    decision: decision.clone(),
    sequence: stored.sequence,
    at: stored.at,
  });
  let shown = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
    EntryKind::Command(shown) if entry.invocation == stored.invocation && *shown.item == **item => {
      Some(&mut shown.mark)
    }
    EntryKind::McpTool(shown) if entry.invocation == stored.invocation && *shown.item == **item => {
      Some(&mut shown.mark)
    }
    _ => None,
  });
  match shown {
    Some(shown) => *shown = Some(mark),
    None if command => early.push((stored.invocation, item.as_str().into(), mark)),
    None => mcp::push_mcp_tool(
      entries,
      stored,
      McpToolEntry {
        item: item.as_str().into(),
        server: server.as_str().into(),
        tool: action.as_str().into(),
        mark: Some(mark),
        status: None,
        native: None,
      },
    ),
  }
}

/// Marks the latest entry of `command` in the same invocation as declined, or adds a declined one
/// when the command was not shown: a native agent's command that did not run is never shown before.
fn deny_command(entries: &mut Vec<Entry>, early: &mut Early, stored: &ActivityEntry, (item, command): (&str, &str)) {
  let native = early.told.take(stored.invocation, item);
  let shown = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
    EntryKind::Command(shown)
      if native.is_none() && entry.invocation == stored.invocation && *shown.command == *command =>
    {
      Some(shown)
    }
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
        mark: early.mark(stored.invocation, item),
        native,
      }),
    }),
  }
}

/// Completes the entry its start created, or adds one when the start was not seen.
fn finish_command(entries: &mut Vec<Entry>, early: &mut Early, stored: &ActivityEntry) {
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
  // The start of the same call: by its item, which a CLI may finish under other words than it started with
  // (a declined escalation), else by its command.
  let open = |entry: &Entry, same: &dyn Fn(&CommandEntry) -> bool| {
    entry.invocation == stored.invocation
      && matches!(&entry.kind, EntryKind::Command(started) if started.status.is_none() && same(started))
  };
  let index = entries
    .iter()
    .rposition(|entry| open(entry, &|started| *started.item == **item))
    .or_else(|| {
      entries
        .iter()
        .rposition(|entry| open(entry, &|started| *started.command == **command))
    });
  let started = index.map(|index| &mut entries[index]);
  // A call keeps the command it started with: that is what was asked about and decided.
  let command: Arc<str> = match started.as_deref().map(|entry| &entry.kind) {
    Some(EntryKind::Command(started)) => started.command.clone(),
    _ => command.as_str().into(),
  };
  let finished = |duration, (mark, native)| {
    EntryKind::Command(CommandEntry {
      item: item.as_str().into(),
      command: command.clone(),
      output: output.as_str().into(),
      status: Some(*status),
      exit_code: *exit_code,
      duration,
      mark,
      native,
    })
  };
  match started {
    Some(entry) => {
      let started = match &entry.kind {
        EntryKind::Command(started) => (started.mark.clone(), started.native.clone()),
        _ => (None, None),
      };
      entry.kind = finished(Some(stored.at.unix_millis() - entry.at.unix_millis()), started);
    }
    None => {
      let native = early.told.take(stored.invocation, item);
      entries.push(Entry {
        sequence: stored.sequence,
        part: 0,
        execution: stored.execution,
        invocation: stored.invocation,
        at: stored.at,
        kind: finished(None, (None, native)),
      });
    }
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
