//! A person's words added to a denial (*Deny with a message*, `D2fdTQ`): the agent gets them as the denial's
//! reason. They are the person's own text, not a secret, kept with the decision and shown in the conversation.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// What a person tells the agent with a denial: trimmed, not empty, at most [`DenialNote::MAX_CHARS`]
/// characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DenialNote(String);

impl DenialNote {
  /// The most characters a note holds: a few paragraphs, so a denial never floods the agent's context.
  pub const MAX_CHARS: usize = 2000;

  pub fn new(text: &str) -> Result<Self, DenialNoteError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(DenialNoteError::Empty);
    }
    let chars = text.chars().count();
    if chars > Self::MAX_CHARS {
      return Err(DenialNoteError::TooLong { chars });
    }
    Ok(Self(text.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for DenialNote {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for DenialNote {
  type Error = DenialNoteError;

  fn try_from(text: String) -> Result<Self, DenialNoteError> {
    Self::new(&text)
  }
}

impl From<DenialNote> for String {
  fn from(note: DenialNote) -> Self {
    note.0
  }
}

/// Words a denial cannot carry.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DenialNoteError {
  #[error("a denial's message has no words")]
  Empty,
  #[error("a denial's message holds {chars} characters, more than {max}", max = DenialNote::MAX_CHARS)]
  TooLong { chars: usize },
}
