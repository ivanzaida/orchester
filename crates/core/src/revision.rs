use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RevisionError {
  #[error("a revision is a full commit SHA of {len} lowercase hex digits, got `{0}`", len = Revision::LEN)]
  Invalid(String),
}

/// A full 40-character git commit SHA. Abbreviated SHAs are display only; stored references
/// are always complete so they cannot become ambiguous.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Revision(String);

impl Revision {
  pub const LEN: usize = 40;

  pub fn new(value: &str) -> Result<Self, RevisionError> {
    let valid = value.len() == Self::LEN && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(RevisionError::Invalid(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for Revision {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl FromStr for Revision {
  type Err = RevisionError;

  fn from_str(value: &str) -> Result<Self, RevisionError> {
    Self::new(value)
  }
}

impl TryFrom<String> for Revision {
  type Error = RevisionError;

  fn try_from(value: String) -> Result<Self, RevisionError> {
    Self::new(&value)
  }
}

impl From<Revision> for String {
  fn from(revision: Revision) -> Self {
    revision.0
  }
}
