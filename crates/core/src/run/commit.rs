use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommitIdError {
  #[error("a commit id is 40 or 64 lowercase hexadecimal digits, got `{0}`")]
  Invalid(String),
}

/// A Git object id in full: 40 lowercase hex digits in a SHA-1 repository, 64 in a SHA-256 one. Abbreviated ids and
/// ref names are refused, so two ids of the same commit are always equal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommitId(String);

impl CommitId {
  pub fn new(value: &str) -> Result<Self, CommitIdError> {
    let valid =
      matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(CommitIdError::Invalid(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for CommitId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for CommitId {
  type Error = CommitIdError;

  fn try_from(value: String) -> Result<Self, CommitIdError> {
    Self::new(&value)
  }
}

impl From<CommitId> for String {
  fn from(commit: CommitId) -> Self {
    commit.0
  }
}
