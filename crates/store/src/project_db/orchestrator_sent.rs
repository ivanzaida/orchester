//! The columns a message or an answer keeps of what the Orchestrator chat wrote (ORC-45): whether it wrote it
//! (`by_orchestrator`) and its conversation (`orchestrator_chat`, only on a row the chat wrote). How it went out is
//! who decided it, kept beside them (`by_automation`, `answered_by`).

use orchester_core::id::ChatId;
use rusqlite::Row;

use crate::{StoreError, rows::parse};

/// The two columns, as a row holds them.
pub(super) struct OrchestratorColumns {
  pub(super) by: bool,
  chat: Option<String>,
}

impl OrchestratorColumns {
  /// The columns of `row` at `by` and `chat`.
  pub(super) fn read(row: &Row<'_>, (by, chat): (usize, usize)) -> rusqlite::Result<Self> {
    Ok(Self {
      by: row.get(by)?,
      chat: row.get(chat)?,
    })
  }

  /// The conversation the chat wrote it in, as kept; `None` for one kept before it was.
  pub(super) fn chat(&self) -> Result<Option<ChatId>, StoreError> {
    self
      .chat
      .as_deref()
      .map(|chat| parse("orchestrator chat id", chat))
      .transpose()
  }
}
