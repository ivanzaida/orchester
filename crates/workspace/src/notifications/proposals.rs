//! What the project's Orchestrator conversations hold for the notifications: the proposals that wait for the person
//! (`orchestrator::proposal`), a start or a change its agent proposed that no decision took yet; and the runs the
//! chat merged itself, which its conversation tells and no OS notification does.

use std::path::Path;

use orchester_core::{
  id::{ChatId, RunId},
  orchestrator::{ActionOutcome, ChatAction, ChatEntry, EntryKind, change::Change},
};
use orchester_store::{ProjectDb, StoreError};

/// A proposal waiting for the person: its entry in its conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitingProposal {
  pub chat: ChatId,
  pub seq: i64,
}

/// What every conversation of a project holds for the notifications.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatReading {
  pub proposals: Vec<WaitingProposal>,
  /// The runs the chat merged by itself (`merge_run`, carried out without a person's *Apply*).
  pub merged: Vec<RunId>,
}

/// What the conversations of the project at `root` hold. Blocks on SQLite.
pub fn chats_in(root: &Path) -> Result<ChatReading, StoreError> {
  let db = ProjectDb::open(root)?;
  let mut reading = ChatReading::default();
  for listing in db.chats()? {
    let chat = listing.chat.id();
    for entry in db.chat_entries(chat)? {
      if waits(&entry) {
        reading.proposals.push(WaitingProposal { chat, seq: entry.seq });
      }
      if let EntryKind::Action(action) = &entry.kind {
        reading.merged.extend(merged_by_chat(action));
      }
    }
  }
  Ok(reading)
}

fn waits(entry: &ChatEntry) -> bool {
  matches!(&entry.kind, EntryKind::Action(action) if action.waits())
}

/// The run `action` merged, when the chat carried the merge out itself; one a person applied is theirs.
pub fn merged_by_chat(action: &ChatAction) -> Option<RunId> {
  if !matches!(action.outcome, ActionOutcome::Done) {
    return None;
  }
  match action.change.as_deref().map(|change| &change.change) {
    Some(Change::Merge { run, .. }) => Some(*run),
    _ => None,
  }
}
