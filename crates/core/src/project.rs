use std::{fmt, num::NonZeroU32};

use thiserror::Error;

use crate::{id::ProjectId, name::Name};

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

  /// A key derived from a project name, so the user does not have to invent one: the
  /// initials of a multi-word name (`Personal website` → `PW`), otherwise the first three
  /// characters (`Kontur` → `KON`). Only words starting with an ASCII letter count; a name
  /// without any falls back to `PRJ`. The user can change the key later.
  pub fn suggest(name: &Name) -> Self {
    const FALLBACK: &str = "PRJ";
    const SINGLE_WORD_CHARS: usize = 3;
    const MAX_INITIALS: usize = 4;
    let words: Vec<String> = name
      .as_str()
      .split(|c: char| !c.is_ascii_alphanumeric())
      .map(|word| word.to_ascii_uppercase())
      .filter(|word| word.starts_with(|c: char| c.is_ascii_alphabetic()))
      .collect();
    let candidate: String = match words.as_slice() {
      [single] => single.chars().take(SINGLE_WORD_CHARS).collect(),
      several => several
        .iter()
        .filter_map(|word| word.chars().next())
        .take(MAX_INITIALS)
        .collect(),
    };
    Self::new(&candidate).unwrap_or_else(|_| Self(FALLBACK.to_owned()))
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

/// A repository managed by Orchester. Everything operational belongs to exactly one project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
  id: ProjectId,
  key: ProjectKey,
  name: Name,
}

impl Project {
  pub const fn new(id: ProjectId, key: ProjectKey, name: Name) -> Self {
    Self { id, key, name }
  }

  pub const fn id(&self) -> ProjectId {
    self.id
  }

  pub const fn key(&self) -> &ProjectKey {
    &self.key
  }

  pub const fn name(&self) -> &Name {
    &self.name
  }
}
