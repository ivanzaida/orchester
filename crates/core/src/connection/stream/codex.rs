//! `codex exec --json` output: one JSON object per line (`.tmp/cli-probe/REPORT.md`).
//! `thread.started{thread_id}`, `turn.started`, `item.started`/`item.completed` with an `item`
//! of type `agent_message`, `reasoning`, `command_execution` or `file_change`,
//! `turn.completed{usage}`, `error{message}` and `turn.failed{error{message}}`. Error messages
//! are often a JSON document themselves; its innermost `message` is what a person reads.

use serde_json::Value;

use super::{AgentEvent, CommandStatus, FileChange, FileChangeKind, OutputStream};
use crate::run::TokenUsage;

/// The event one line of Codex output reports. Standard error carries Codex's own diagnostics,
/// which are kept as raw lines.
pub fn parse_codex_line(stream: OutputStream, line: &str) -> AgentEvent {
  let raw = || AgentEvent::Raw {
    stream,
    line: line.to_owned(),
  };
  if stream == OutputStream::Stderr {
    return raw();
  }
  let Ok(value) = serde_json::from_str::<Value>(line) else {
    return raw();
  };
  parse_event(&value).unwrap_or_else(raw)
}

fn parse_event(value: &Value) -> Option<AgentEvent> {
  match value.get("type")?.as_str()? {
    "thread.started" => Some(AgentEvent::SessionStarted {
      session: text(value, "thread_id")?,
    }),
    "turn.started" => Some(AgentEvent::TurnStarted),
    "turn.completed" => Some(AgentEvent::TurnCompleted {
      usage: usage(value.get("usage")?)?,
    }),
    "turn.failed" => Some(AgentEvent::TurnFailed {
      message: readable(&text(value.get("error")?, "message")?),
    }),
    "error" => Some(AgentEvent::Error {
      message: readable(&text(value, "message")?),
    }),
    "item.started" => started_item(value.get("item")?),
    "item.completed" => completed_item(value.get("item")?),
    _ => None,
  }
}

fn started_item(item: &Value) -> Option<AgentEvent> {
  match item.get("type")?.as_str()? {
    "command_execution" => Some(AgentEvent::CommandStarted {
      item: text(item, "id")?,
      command: text(item, "command")?,
    }),
    _ => None,
  }
}

fn completed_item(item: &Value) -> Option<AgentEvent> {
  match item.get("type")?.as_str()? {
    "agent_message" => Some(AgentEvent::Message {
      text: text(item, "text")?,
    }),
    "reasoning" => Some(AgentEvent::Reasoning {
      text: text(item, "text")?,
    }),
    "command_execution" => Some(AgentEvent::CommandFinished {
      item: text(item, "id")?,
      command: text(item, "command")?,
      output: item
        .get("aggregated_output")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned(),
      exit_code: match item.get("exit_code")? {
        Value::Null => None,
        code => Some(code.as_i64()?),
      },
      status: command_status(item.get("status")?.as_str()?)?,
    }),
    "file_change" => Some(AgentEvent::FileChanged {
      item: text(item, "id")?,
      changes: item
        .get("changes")?
        .as_array()?
        .iter()
        .map(file_change)
        .collect::<Option<_>>()?,
    }),
    _ => None,
  }
}

fn command_status(status: &str) -> Option<CommandStatus> {
  match status {
    "completed" => Some(CommandStatus::Completed),
    "failed" => Some(CommandStatus::Failed),
    "declined" => Some(CommandStatus::Declined),
    _ => None,
  }
}

fn file_change(change: &Value) -> Option<FileChange> {
  let kind = match change.get("kind")?.as_str()? {
    "add" => FileChangeKind::Added,
    "update" => FileChangeKind::Modified,
    "delete" => FileChangeKind::Deleted,
    _ => return None,
  };
  Some(FileChange {
    path: text(change, "path")?,
    kind,
  })
}

fn usage(usage: &Value) -> Option<TokenUsage> {
  let count = |name| usage.get(name).and_then(Value::as_u64);
  Some(TokenUsage {
    input: count("input_tokens")?,
    cached_input: count("cached_input_tokens").unwrap_or(0),
    output: count("output_tokens")?,
  })
}

fn text(value: &Value, name: &str) -> Option<String> {
  value.get(name)?.as_str().map(str::to_owned)
}

/// The innermost `message` of an error that is itself a JSON document, e.g.
/// `{"error":{"message":"The model is not supported"}}`; other text as it is.
fn readable(message: &str) -> String {
  let mut current: Option<Value> = serde_json::from_str(message).ok();
  let mut found = None;
  while let Some(value) = current {
    let next = value.get("error").cloned();
    if let Some(text) = value.get("message").and_then(Value::as_str) {
      found = Some(text.to_owned());
    }
    current = next.filter(Value::is_object);
  }
  found.unwrap_or_else(|| message.to_owned())
}
