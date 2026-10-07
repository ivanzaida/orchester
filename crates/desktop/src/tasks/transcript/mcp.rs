//! MCP tool calls in the transcript: a call of an MCP tool that runs without asking marks its command (Claude
//! Code), or is an entry of its own (Codex, which reports no command for it), as is a Codex call that ended or was
//! declined at once. A Codex call of one of Orchester's file tools carries what its input asked ([`NativeCall`]),
//! so that its row reads as the native agent's.

use std::sync::Arc;

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, NativeCall},
  run::ActivityEntry,
};

use super::{CommandMark, Entry, EntryKind, native::Told};

/// An MCP tool call that no command shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpToolEntry {
  /// The CLI's id of the call.
  pub item: Arc<str>,
  pub server: Arc<str>,
  pub tool: Arc<str>,
  /// How it was decided, when the CLI asked or Orchester answered for it.
  pub mark: Option<CommandMark>,
  /// `None` until the call ends.
  pub status: Option<CommandStatus>,
  /// What a call of one of Orchester's file tools asked; `None` for any other call.
  pub native: Option<NativeCall>,
}

/// Marks the command of a call nobody was asked about, or adds an entry for the call when no command
/// shows it.
pub(super) fn not_asked(entries: &mut Vec<Entry>, stored: &ActivityEntry) {
  let (item, server, tool, mark) = match &stored.event {
    AgentEvent::McpToolNotAsked { item, server, tool } => (
      item,
      server,
      tool,
      CommandMark::ExemptTool {
        server: server.as_str().into(),
      },
    ),
    AgentEvent::McpToolDeclined { item, server, tool } => (
      item,
      server,
      tool,
      CommandMark::Declined {
        server: server.as_str().into(),
      },
    ),
    _ => return,
  };
  let shown = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
    EntryKind::Command(shown) if entry.invocation == stored.invocation && *shown.item == **item => Some(shown),
    _ => None,
  });
  match shown {
    Some(shown) => shown.mark = Some(mark),
    None => push_mcp_tool(
      entries,
      stored,
      McpToolEntry {
        item: item.as_str().into(),
        server: server.as_str().into(),
        tool: tool.as_str().into(),
        mark: Some(mark),
        status: None,
        native: None,
      },
    ),
  }
}

/// Ends the entry of a Codex MCP call, or adds one: a call nobody was asked about shows only when it ends. What a
/// call of Orchester's file tools asked, told before its end, goes with it.
pub(super) fn mcp_tool_finished(entries: &mut Vec<Entry>, told: &mut Told, stored: &ActivityEntry) {
  let AgentEvent::McpToolFinished {
    item,
    server,
    tool,
    status,
    asked,
  } = &stored.event
  else {
    return;
  };
  let server: Arc<str> = server.as_str().into();
  let decided = asked.then(|| match status {
    CommandStatus::Completed => CommandMark::ApprovedByYou { server: server.clone() },
    _ => CommandMark::AskedYou { server: server.clone() },
  });
  let shown = entries.iter_mut().rev().find_map(|entry| match &mut entry.kind {
    EntryKind::McpTool(shown) if entry.invocation == stored.invocation && *shown.item == **item => Some(shown),
    _ => None,
  });
  match shown {
    Some(shown) => {
      shown.status = Some(*status);
      shown.mark = shown.mark.take().or(decided);
    }
    None => push_mcp_tool(
      entries,
      stored,
      McpToolEntry {
        item: item.as_str().into(),
        native: told.take(stored.invocation, item),
        server,
        tool: tool.as_str().into(),
        mark: decided,
        status: Some(*status),
      },
    ),
  }
}

pub(super) fn push_mcp_tool(entries: &mut Vec<Entry>, stored: &ActivityEntry, call: McpToolEntry) {
  entries.push(Entry {
    sequence: stored.sequence,
    part: 0,
    execution: stored.execution,
    invocation: stored.invocation,
    at: stored.at,
    kind: EntryKind::McpTool(call),
  });
}
