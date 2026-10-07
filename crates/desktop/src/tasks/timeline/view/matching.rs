//! What a search matches (`docs/design/orchester-workspace.md`, "Filters and search"): message text,
//! commands, paths and output, in lower case. What agents and the person write is found as written
//! and as the chat shows it, its Markdown rendered.

use super::super::{Item, ItemKind, mark::MarkKind};
use crate::{
  tasks::transcript::{Entry, EntryKind},
  ui::markdown::plain::{has_markup, plain_text},
};

/// Whether what `item` says contains `needle`, in lower case.
pub(super) fn matches(item: &Item, needle: &str) -> bool {
  let found = |text: &str| text.to_lowercase().contains(needle);
  let written = |text: &str| found(text) || (has_markup(text) && found(&plain_text(text)));
  match &item.kind {
    ItemKind::AgentMessage { text, .. } => written(text),
    ItemKind::Message(entry) | ItemKind::Question { entry, .. } => written(&entry.body),
    ItemKind::Plan(card) => written(&card.plan.text),
    ItemKind::Approval(card) => [
      Some(card.request.action.as_str()),
      Some(card.request.server.as_str()),
      card.request.target.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(found),
    ItemKind::Problem(entry) => entry_matches(entry, needle),
    ItemKind::Result(card) => card.reported.as_deref().is_some_and(written),
    ItemKind::Tools(group) => group.entries.iter().any(|entry| entry_matches(entry, needle)),
    ItemKind::Mark(mark) => match &mark.kind {
      MarkKind::ReturnedWithNote { note, .. } => written(note),
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
    EntryKind::Approval(approval) => found(&approval.command),
    EntryKind::Usage(_) | EntryKind::Problem(_) => false,
  }
}
