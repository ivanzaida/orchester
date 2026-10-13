//! The Orchestrator chat's conversations as the screen reads them: History's list and one conversation's entries.

use std::path::Path;

use orchester_core::{
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, ChatEntry, EntryKind, TurnState},
};
use orchester_store::{ChatListing, ProjectDb};

use crate::messages::{self, Message};

/// Every conversation of the project at `root`, the one with the latest entry first. Blocks on SQLite.
pub fn history_in(root: &Path) -> Result<Vec<ChatListing>, Message> {
  ProjectDb::open(root)
    .and_then(|db| db.chats())
    .map_err(|error| messages::store_error(&error))
}

/// One conversation and its entries in the order they were kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatView {
  pub chat: Chat,
  pub entries: Vec<ChatEntry>,
}

impl ChatView {
  /// Conversation `chat` of the project at `root`; `None` when it is not kept. Blocks on SQLite.
  pub fn load_in(root: &Path, chat: ChatId) -> Result<Option<Self>, Message> {
    let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
    let Some(found) = db.chat(chat).map_err(|error| messages::store_error(&error))? else {
      return Ok(None);
    };
    let entries = db.chat_entries(chat).map_err(|error| messages::store_error(&error))?;
    Ok(Some(Self { chat: found, entries }))
  }

  /// The person's last message and its turn.
  pub fn last_turn(&self) -> Option<(ChatMessageId, &TurnState)> {
    self.entries.iter().rev().find_map(|entry| match &entry.kind {
      EntryKind::Person { message, turn, .. } => Some((*message, turn)),
      _ => None,
    })
  }

  /// Whether the agent is answering the last message.
  pub fn thinking(&self) -> bool {
    self.last_turn().is_some_and(|(_, turn)| turn.is_thinking())
  }
}
