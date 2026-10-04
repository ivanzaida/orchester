//! What a search matches (`docs/design/orchester-workspace.md`, "Filters and search"): message text,
//! commands, paths and output, in lower case.

use super::super::{Item, ItemKind, mark::MarkKind};
use crate::tasks::transcript::{Entry, EntryKind};

/// Whether what `item` says contains `needle`, in lower case.
pub(super) fn matches(item: &Item, needle: &str) -> bool {
  let found = |text: &str| text.to_lowercase().contains(needle);
  match &item.kind {
    ItemKind::AgentMessage { text, .. } => found(text),
    ItemKind::Message(entry) | ItemKind::Question { entry, .. } => found(&entry.body),
    ItemKind::Plan(card) => found(&card.plan.text),
    ItemKind::Approval(card) => [
      Some(card.request.action.as_str()),
      Some(card.request.server.as_str()),
      card.request.target.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(found),
    ItemKind::Problem(entry) => entry_matches(entry, needle),
    ItemKind::Result(card) => card.reported.as_deref().is_some_and(found),
    ItemKind::Tools(group) => group.entries.iter().any(|entry| entry_matches(entry, needle)),
    ItemKind::Mark(mark) => match &mark.kind {
      MarkKind::ReturnedWithNote { note, .. } => found(note),
      MarkKind::Returned { .. } | MarkKind::Restarted { .. } => false,
    },
    ItemKind::Step(_) | ItemKind::End(_) => false,
  }
}

/// Whether a transcript entry's message, command, path or output contains `needle`.
pub(super) fn entry_matches(entry: &Entry, needle: &str) -> bool {
  let found = |text: &str| text.to_lowercase().contains(needle);
  match &entry.kind {
    EntryKind::Message(text) | EntryKind::Reasoning(text) | EntryKind::Error(text) | EntryKind::NotSignedIn(text) => {
      found(text)
    }
    EntryKind::Command(command) => found(&command.command) || found(&command.output),
    EntryKind::ToolCommand(command) => found(&command.command) || found(&command.output),
    EntryKind::FileChange(change) => found(&change.path),
    EntryKind::Raw(lines) => lines.iter().any(|(_, line)| found(line)),
    EntryKind::McpTool(call) => found(&call.server) || found(&call.tool),
    EntryKind::McpServer(server, _) => server.as_deref().is_some_and(found),
    EntryKind::ExemptionNotApplied { server, tool, .. } => found(server) || found(tool),
    EntryKind::Usage(_) | EntryKind::Problem(_) => false,
  }
}
