//! The Activity events a native invocation emits: the same [`AgentEvent`]s the CLIs produce (§2.2).

use super::{LoopSettings, ToolOutcome, ToolWork};
use crate::{
  connection::stream::{AgentEvent, CommandStatus},
  native::{
    item::{Item, ToolCallId},
    model::ModelResponse,
  },
};

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
pub(super) fn label(work: &ToolWork) -> String {
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

pub(super) fn started(call: &ToolCallId, label: &str) -> AgentEvent {
  AgentEvent::CommandStarted {
    item: call.to_string(),
    command: label.to_owned(),
  }
}

/// The finished call, and the files it changed.
pub(super) fn finished(call: &ToolCallId, label: &str, outcome: &ToolOutcome) -> Vec<AgentEvent> {
  let status = if outcome.is_error {
    CommandStatus::Failed
  } else {
    CommandStatus::Completed
  };
  let mut events = vec![AgentEvent::CommandFinished {
    item: call.to_string(),
    command: label.to_owned(),
    output: outcome.activity.clone(),
    exit_code: outcome.exit_code,
    status,
  }];
  if !outcome.changes.is_empty() {
    events.push(AgentEvent::FileChanged {
      item: call.to_string(),
      changes: outcome.changes.clone(),
    });
  }
  events
}

/// A call whose input could not be read: it failed without running.
pub(super) fn invalid(call: &ToolCallId, name: &str, error: &str) -> AgentEvent {
  AgentEvent::CommandFinished {
    item: call.to_string(),
    command: name.to_owned(),
    output: error.to_owned(),
    exit_code: None,
    status: CommandStatus::Failed,
  }
}

pub(super) fn denied(call: &ToolCallId, command: &str) -> AgentEvent {
  AgentEvent::PermissionDenied {
    item: call.to_string(),
    command: command.to_owned(),
  }
}
