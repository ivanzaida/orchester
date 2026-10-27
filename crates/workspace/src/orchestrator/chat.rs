//! The Orchestrator chat's conversations as the screen reads them: History's list and one conversation's entries,
//! with the files the person attached to their messages and which of its waits still stand.

use std::{
  collections::{BTreeMap, HashSet},
  path::Path,
};

use orchester_core::{
  attachment::Attachment,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, ChatEntry, TurnState},
};
use orchester_store::{ChatListing, ProjectDb};

use crate::messages::{self, Message};

/// The project's conversations as History lists them, the one with the latest entry first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct History {
  pub chats: Vec<ChatListing>,
}

/// Every conversation of the project at `root`. Blocks on SQLite.
pub fn history_in(root: &Path) -> Result<History, Message> {
  ProjectDb::open(root)
    .and_then(|db| db.chats())
    .map(|chats| History { chats })
    .map_err(|error| messages::store_error(&error))
}

/// One conversation and its entries in the order they were kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatView {
  pub chat: Chat,
  pub entries: Vec<ChatEntry>,
  /// The files attached to the person's messages, by message, in the order they were attached.
  pub attachments: BTreeMap<ChatMessageId, Vec<Attachment>>,
  /// The entries of its waits and runs' ends that still stand as they were read: an agent that still waits, a run
  /// still where it ended. The others were dealt with since, here or elsewhere.
  pub standing: HashSet<i64>,
}

impl ChatView {
  /// Conversation `chat` of the project at `root`; `None` when it is not kept. Blocks on SQLite.
  pub fn load_in(root: &Path, chat: ChatId) -> Result<Option<Self>, Message> {
    let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
    let Some(found) = db.chat(chat).map_err(|error| messages::store_error(&error))? else {
      return Ok(None);
    };
    let entries = db.chat_entries(chat).map_err(|error| messages::store_error(&error))?;
    let attachments = db
      .chat_attachments(chat)
      .map_err(|error| messages::store_error(&error))?;
    let standing = super::waits::standing(&db, chat, &entries).map_err(|error| messages::store_error(&error))?;
    Ok(Some(Self {
      chat: found,
      entries,
      attachments,
      standing,
    }))
  }

  /// The files attached to the person's message `message`.
  pub fn attachments_of(&self, message: ChatMessageId) -> &[Attachment] {
    self.attachments.get(&message).map_or(&[], Vec::as_slice)
  }

  /// The last turn and where it stands: a person's message's, or one a wait of an agent started by itself.
  pub fn last_turn(&self) -> Option<(ChatMessageId, &TurnState)> {
    self.entries.iter().rev().find_map(|entry| entry.kind.turn())
  }

  /// Whether the agent is answering the last message.
  pub fn thinking(&self) -> bool {
    self.last_turn().is_some_and(|(_, turn)| turn.is_thinking())
  }
}
