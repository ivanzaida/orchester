//! What a person changes of an Orchestrator conversation in History (ORC-328): its title, which wins over the one
//! taken from its first message from then on, and its pin to History's top.

use orchester_core::{id::ChatId, orchestrator::ChatTitle, time::Timestamp};
use rusqlite::params;

use super::ProjectDb;
use crate::StoreError;

impl ProjectDb {
  /// Titles conversation `chat` `title` as a person named it. Returns whether the project keeps the conversation.
  pub fn rename_chat(&mut self, chat: ChatId, title: &ChatTitle) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_chats SET person_title = ?2 WHERE id = ?1",
      params![chat.to_string(), title.as_str()],
    )?;
    Ok(changed == 1)
  }

  /// Pins conversation `chat` to History's top as pinned at `at`, or unpins it (`None`). Returns whether the project
  /// keeps the conversation.
  pub fn pin_chat(&mut self, chat: ChatId, at: Option<Timestamp>) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE orchestrator_chats SET pinned_at = ?2 WHERE id = ?1",
      params![chat.to_string(), at.map(|at| at.unix_millis())],
    )?;
    Ok(changed == 1)
  }
}
