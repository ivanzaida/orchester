//! The Orchestrator's proposals that wait for the person (`orchestrator::proposal`): a start or a change its agent
//! proposed that no decision took yet.

use std::path::Path;

use orchester_core::{
  id::ChatId,
  orchestrator::{ChatEntry, EntryKind},
};
use orchester_store::{ProjectDb, StoreError};

/// A proposal waiting for the person: its entry in its conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitingProposal {
  pub chat: ChatId,
  pub seq: i64,
}

/// The proposals of every conversation of the project at `root` that wait for the person. Blocks on SQLite.
pub fn waiting_proposals_in(root: &Path) -> Result<Vec<WaitingProposal>, StoreError> {
  let db = ProjectDb::open(root)?;
  let mut waiting = Vec::new();
  for listing in db.chats()? {
    let chat = listing.chat.id();
    waiting.extend(
      db.chat_entries(chat)?
        .iter()
        .filter(|entry| waits(entry))
        .map(|entry| WaitingProposal { chat, seq: entry.seq }),
    );
  }
  Ok(waiting)
}

fn waits(entry: &ChatEntry) -> bool {
  matches!(&entry.kind, EntryKind::Action(action) if action.waits())
}
