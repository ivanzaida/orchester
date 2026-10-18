//! The columns a message or an answer keeps of what the Orchestrator chat wrote (ORC-45): whether it wrote it
//! (`by_orchestrator`), its conversation (`orchestrator_chat`) and how it went out (`orchestrator_way`). Only a row
//! the chat wrote holds the latter two.

use orchester_core::orchestrator::{FromOrchestrator, SentWay};
use rusqlite::Row;

use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The three columns, as a row holds them.
pub(super) struct OrchestratorColumns {
  pub(super) by: bool,
  pub(super) chat: Option<String>,
  pub(super) way: Option<String>,
}

impl OrchestratorColumns {
  /// The columns of `from`: none for a row the chat did not write.
  pub(super) fn of(from: Option<FromOrchestrator>) -> Self {
    Self {
      by: from.is_some(),
      chat: from.and_then(|from| from.chat).map(|chat| chat.to_string()),
      way: from.and_then(|from| from.way).map(|way| way.word().to_owned()),
    }
  }

  /// The columns of `row` from its column `first` on.
  pub(super) fn read(row: &Row<'_>, first: usize) -> rusqlite::Result<Self> {
    Ok(Self {
      by: row.get(first)?,
      chat: row.get(first + 1)?,
      way: row.get(first + 2)?,
    })
  }

  /// What the chat wrote, as kept; `None` for a row it did not write.
  pub(super) fn from(self) -> Result<Option<FromOrchestrator>, StoreError> {
    if !self.by {
      return Ok(None);
    }
    let way = self
      .way
      .map(|word| SentWay::of_word(&word).ok_or_else(|| corrupt("orchestrator way", word)))
      .transpose()?;
    Ok(Some(FromOrchestrator {
      chat: self.chat.map(|chat| parse("orchestrator chat id", &chat)).transpose()?,
      way,
    }))
  }
}
