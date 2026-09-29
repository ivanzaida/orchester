use std::{fmt, num::NonZeroU32};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::TaskKey;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BranchNameError {
  #[error("`{0}` is not a valid git branch name")]
  Invalid(String),
}

/// A git branch name, checked against the parts of `git check-ref-format --branch` that can be
/// decided without a repository.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BranchName(String);

impl BranchName {
  pub const MAX_BYTES: usize = 255;

  pub fn new(value: &str) -> Result<Self, BranchNameError> {
    const FORBIDDEN_CHARS: &[char] = &['~', '^', ':', '?', '*', '[', '\\'];
    const FORBIDDEN_PARTS: &[&str] = &["..", "@{", "//", "/.", ".lock/"];
    let valid = !value.is_empty()
      && value.len() <= Self::MAX_BYTES
      && value != "@"
      && !value.starts_with(['-', '/', '.'])
      && !value.ends_with(['/', '.'])
      && !value.ends_with(".lock")
      && !value
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || FORBIDDEN_CHARS.contains(&c))
      && !FORBIDDEN_PARTS.iter().any(|part| value.contains(part));
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(BranchNameError::Invalid(value.to_owned()))
    }
  }

  /// The branch of the `ordinal`-th run of task `task` (from 1): `orchester/orc-12-run-1`.
  pub fn for_run(task: &TaskKey, ordinal: NonZeroU32) -> Self {
    let name = format!("orchester/{}-run-{ordinal}", task.to_string().to_ascii_lowercase());
    Self::new(&name).expect("a lowercase task key, digits, dashes and one slash form a valid branch name")
  }

  /// The branch of the run a task with `earlier` runs (readable or not) starts next: runs of a
  /// task are numbered on from its last one, so each has a branch of its own.
  pub fn for_next_run(task: &TaskKey, earlier: usize) -> Self {
    let ordinal = u32::try_from(earlier + 1)
      .ok()
      .and_then(NonZeroU32::new)
      .expect("fewer than 2^32 runs of one task");
    Self::for_run(task, ordinal)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for BranchName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for BranchName {
  type Error = BranchNameError;

  fn try_from(value: String) -> Result<Self, BranchNameError> {
    Self::new(&value)
  }
}

impl From<BranchName> for String {
  fn from(branch: BranchName) -> Self {
    branch.0
  }
}
