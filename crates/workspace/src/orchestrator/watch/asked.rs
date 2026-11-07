//! `/watching` (frame 06 `Ff7AD`): the person asks what the conversation leads and what waits there. The app answers
//! it itself, with no agent turn and nothing spent: the command is kept as the person's entry, and the answer under it
//! is read from the live state whenever it is shown (`super::super::watching::WatchReads::watching`).

use std::path::Path;

use orchester_core::{
  orchestrator::{Chat, ChatEvent, watch::WatchCommand},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::WatchError;

/// Keeps `chat`'s `/watching` in the project at `root`. Refused when the conversation watches nothing, which the
/// command has nothing to answer about. Blocks on SQLite.
pub fn watching_in(root: &Path, chat: &Chat) -> Result<(), WatchError> {
  let mut db = ProjectDb::open(root)?;
  if db.active_subscriptions()?.of_chat(chat.id()).next().is_none() {
    return Err(WatchError::NotWatched);
  }
  let command = WatchCommand::Watching;
  db.keep_watch_entries(chat, &[ChatEvent::WatchCommand { command }], Timestamp::now())?;
  Ok(())
}
