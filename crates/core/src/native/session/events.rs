//! The Activity events a native invocation emits: the same [`AgentEvent`]s the CLIs produce (§2.2), and
//! before a tool call's end what the call did ([`AgentEvent::NativeCall`]), for its transcript row.

use super::{LoopSettings, ToolOutcome, ToolWork};
use crate::{
  connection::stream::{AgentEvent, CallDetail, CommandStatus, Declined, DeclinedBy, NativeCall},
  native::{
    command::CommandForm,
    item::{Item, ToolCallId},
    model::ModelResponse,
    tools::ToolName,
  },
  run::CommandShell,
};

/// A running call as Activity shows it: its label, its tool and, for a shell command, the shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Shown {
  pub(super) call: ToolCallId,
  label: String,
  tool: ToolName,
  shell: Option<CommandShell>,
}

impl Shown {
  pub(super) fn of(call: &ToolCallId, work: &ToolWork, settings: &LoopSettings) -> Self {
    let (tool, shell) = match work {
      ToolWork::ReadFile(_) => (ToolName::ReadFile, None),
      ToolWork::ListFiles(_) => (ToolName::ListFiles, None),
      ToolWork::Search(_) => (ToolName::Search, None),
      ToolWork::EditFile(_) => (ToolName::EditFile, None),
      ToolWork::WriteFile(_) => (ToolName::WriteFile, None),
      ToolWork::Command(command) => (
        ToolName::RunCommand,
        command_shell(matches!(command.form(), CommandForm::Shell), settings),
      ),
    };
    Self {
      call: call.clone(),
      label: label(work),
      tool,
      shell,
    }
  }
}

/// The shell a command goes to: the invocation's, for a shell command.
pub(super) fn command_shell(shell: bool, settings: &LoopSettings) -> Option<CommandShell> {
  settings.shell.filter(|_| shell)
}

pub(super) fn session(settings: &LoopSettings) -> AgentEvent {
  AgentEvent::SessionStarted {
    session: settings.conversation.to_string(),
  }
}

/// A response's messages and reasoning summaries, then its usage.
pub(super) fn response(response: &ModelResponse) -> Vec<AgentEvent> {
  let mut events: Vec<AgentEvent> = response
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Assistant { text } if !text.trim().is_empty() => Some(AgentEvent::Message { text: text.clone() }),
      Item::Reasoning {
        summary: Some(text), ..
      } => Some(AgentEvent::Reasoning { text: text.clone() }),
      _ => None,
    })
    .collect();
  events.push(AgentEvent::TurnCompleted { usage: response.usage });
  events
}

/// How Activity names a call: a command as written, a file tool by its name and target.
fn label(work: &ToolWork) -> String {
  match work {
    ToolWork::ReadFile(read) => {
      let last = read.offset().saturating_add(read.limit() - 1);
      format!("read_file {}:{}-{last}", read.path(), read.offset())
    }
    ToolWork::ListFiles(list) => {
      let target = [list.path(), list.pattern()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
      if target.is_empty() {
        "list_files".to_owned()
      } else {
        format!("list_files {target}")
      }
    }
    ToolWork::Search(search) => match search.path() {
      Some(path) => format!("search {} {path}", search.pattern()),
      None => format!("search {}", search.pattern()),
    },
    ToolWork::EditFile(edit) => format!("edit_file {}", edit.path()),
    ToolWork::WriteFile(write) => format!("write_file {}", write.path()),
    ToolWork::Command(command) => command.text().to_owned(),
  }
}

pub(super) fn started(shown: &Shown) -> AgentEvent {
  AgentEvent::CommandStarted {
    item: shown.call.to_string(),
    command: shown.label.clone(),
  }
}

/// What the call did, its end, and the files it changed.
pub(super) fn finished(shown: &Shown, outcome: &ToolOutcome) -> Vec<AgentEvent> {
  let status = if outcome.is_error {
    CommandStatus::Failed
  } else {
    CommandStatus::Completed
  };
  let detail = match shown.tool {
    ToolName::RunCommand => Some(CallDetail::Command { shell: shown.shell }),
    _ => outcome.detail.clone(),
  };
  let mut events = vec![
    AgentEvent::NativeCall {
      item: shown.call.to_string(),
      call: NativeCall {
        tool: shown.tool,
        detail,
        declined: None,
      },
    },
    AgentEvent::CommandFinished {
      item: shown.call.to_string(),
      command: shown.label.clone(),
      output: outcome.activity.clone(),
      exit_code: outcome.exit_code,
      status,
    },
  ];
  if !outcome.changes.is_empty() {
    events.push(AgentEvent::FileChanged {
      item: shown.call.to_string(),
      changes: outcome.changes.clone(),
    });
  }
  events
}

/// A call whose input could not be read: it failed without running.
pub(super) fn invalid(call: &ToolCallId, name: &str, error: &str) -> Vec<AgentEvent> {
  let told = ToolName::from_name(name).map(|tool| AgentEvent::NativeCall {
    item: call.to_string(),
    call: NativeCall {
      tool,
      detail: None,
      declined: None,
    },
  });
  told
    .into_iter()
    .chain([AgentEvent::CommandFinished {
      item: call.to_string(),
      command: name.to_owned(),
      output: error.to_owned(),
      exit_code: None,
      status: CommandStatus::Failed,
    }])
    .collect()
}

/// A command that did not run: `by` declined it, and the agent was told `result`.
pub(super) fn declined(
  call: &ToolCallId,
  (command, shell): (&str, Option<CommandShell>),
  by: DeclinedBy,
  result: &str,
) -> Vec<AgentEvent> {
  vec![
    AgentEvent::NativeCall {
      item: call.to_string(),
      call: NativeCall {
        tool: ToolName::RunCommand,
        detail: Some(CallDetail::Command { shell }),
        declined: Some(Declined {
          by,
          result: result.to_owned(),
        }),
      },
    },
    AgentEvent::PermissionDenied {
      item: call.to_string(),
      command: command.to_owned(),
    },
  ]
}
