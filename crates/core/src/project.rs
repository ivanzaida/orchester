use std::{fmt, num::NonZeroU32};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProjectKeyError {
  #[error(
    "project key must be {min}–{max} characters: an uppercase letter followed by uppercase letters or digits, got `{0}`",
    min = ProjectKey::MIN_CHARS,
    max = ProjectKey::MAX_CHARS
  )]
  Invalid(String),
}

/// Short project prefix for display keys, e.g. `ORC` in `ORC-129`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectKey(String);

impl ProjectKey {
  pub const MIN_CHARS: usize = 2;
  pub const MAX_CHARS: usize = 10;

  pub fn new(value: &str) -> Result<Self, ProjectKeyError> {
    let valid_length = (Self::MIN_CHARS..=Self::MAX_CHARS).contains(&value.len());
    let mut chars = value.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_uppercase());
    let rest_valid = chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if valid_length && starts_with_letter && rest_valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(ProjectKeyError::Invalid(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// Display key of a task (`ORC-129`). Tasks are referenced by `TaskId`; this is for people.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskKey {
  project: ProjectKey,
  number: NonZeroU32,
}

impl TaskKey {
  pub const fn new(project: ProjectKey, number: NonZeroU32) -> Self {
    Self { project, number }
  }

  pub fn project(&self) -> &ProjectKey {
    &self.project
  }

  pub const fn number(&self) -> NonZeroU32 {
    self.number
  }
}

impl fmt::Display for TaskKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}-{}", self.project.as_str(), self.number)
  }
}
