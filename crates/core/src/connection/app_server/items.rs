//! What a completed item of Codex App Server reports: an MCP call's end, a command's end, and the files a patch
//! changed.

use serde_json::Value;

use super::{ToolContext, Update, string};
use crate::connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind};

/// The end of the MCP call `item`, started as `started`, as its activity: what a successful call of one of
/// Orchester's file tools asked (its row reads as the native agent's call of the tool, an edit's with its diff), what
/// it called and whether Codex asked first, then the changes a successful call of Orchester's file tools made. A call
/// fails when Codex says so, or when its answer is marked as an error (`result.isError`) where Codex passes that on.
pub(super) fn mcp_tool_finished(item: &Value, started: Option<ToolContext>) -> Vec<Update> {
  let (Some(id), Some(server), Some(tool)) = (
    string(item, "id"),
    started
      .as_ref()
      .map(|tool| tool.server.clone())
      .or_else(|| string(item, "server")),
    started
      .as_ref()
      .map(|tool| tool.action.clone())
      .or_else(|| string(item, "tool")),
  ) else {
    return Vec::new();
  };
  let status = if item.get("status").and_then(Value::as_str) == Some("completed") {
    CommandStatus::Completed
  } else {
    CommandStatus::Failed
  };
  let answered_error = item
    .get("result")
    .and_then(|result| result.get("isError"))
    .and_then(Value::as_bool)
    == Some(true);
  let (asked, changes, call) = started.map_or((false, Vec::new(), None), |tool| (tool.asked, tool.changes, tool.call));
  let succeeded = status == CommandStatus::Completed && !answered_error;
  let told = call
    .filter(|_| succeeded)
    .map(|call| Update::Activity(AgentEvent::NativeCall { item: id.clone(), call }));
  let changed = (succeeded && !changes.is_empty()).then(|| {
    Update::Activity(AgentEvent::FileChanged {
      item: id.clone(),
      changes,
    })
  });
  let finished = Update::Activity(AgentEvent::McpToolFinished {
    item: id,
    server,
    tool,
    status,
    asked,
  });
  told.into_iter().chain([finished]).chain(changed).collect()
}

pub(super) fn command_finished(item: &Value) -> Vec<Update> {
  let status = match item.get("status").and_then(Value::as_str) {
    Some("completed") => CommandStatus::Completed,
    Some("failed") => CommandStatus::Failed,
    Some("declined") => CommandStatus::Declined,
    _ => return Vec::new(),
  };
  match (string(item, "id"), string(item, "command")) {
    (Some(item_id), Some(command)) => vec![Update::Activity(AgentEvent::CommandFinished {
      item: item_id,
      command,
      output: string(item, "aggregatedOutput").unwrap_or_default(),
      exit_code: item.get("exitCode").and_then(Value::as_i64),
      status,
    })],
    _ => Vec::new(),
  }
}

/// The files a completed patch changed. The kind is a word (`"update"`) or, as the protocol writes
/// a patch's change, an object naming it (`{"type": "update"}`).
pub(super) fn file_changed(item: &Value) -> Vec<Update> {
  if item.get("status").and_then(Value::as_str) != Some("completed") {
    return Vec::new();
  }
  let changes: Vec<FileChange> = item
    .get("changes")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(|change| {
      let kind = change.get("kind")?;
      let kind = kind.as_str().or_else(|| kind.get("type")?.as_str())?;
      let kind = match kind {
        "add" => FileChangeKind::Added,
        "update" => FileChangeKind::Modified,
        "delete" => FileChangeKind::Deleted,
        _ => return None,
      };
      Some(FileChange {
        path: string(change, "path")?,
        kind,
      })
    })
    .collect();
  match (string(item, "id"), changes.is_empty()) {
    (Some(item), false) => vec![Update::Activity(AgentEvent::FileChanged { item, changes })],
    _ => Vec::new(),
  }
}
