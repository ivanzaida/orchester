use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NameError {
  #[error("name must not be blank")]
  Blank,
  #[error("name is {length} characters long; the limit is {}", Name::MAX_CHARS)]
  TooLong { length: usize },
}

/// A user-facing name or title: trimmed, not blank, bounded in length.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Name(String);

impl Name {
  pub const MAX_CHARS: usize = 200;

  pub fn new(value: &str) -> Result<Self, NameError> {
    let trimmed = value.trim();
    let length = trimmed.chars().count();
    if length == 0 {
      return Err(NameError::Blank);
    }
    if length > Self::MAX_CHARS {
      return Err(NameError::TooLong { length });
    }
    Ok(Self(trimmed.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for Name {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for Name {
  type Error = NameError;

  fn try_from(value: String) -> Result<Self, NameError> {
    Self::new(&value)
  }
}

impl From<Name> for String {
  fn from(name: Name) -> Self {
    name.0
  }
}
