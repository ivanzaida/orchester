//! A doc's title and the slug its address is made of.

use std::fmt;

use thiserror::Error;

use crate::name::{Name, NameError};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DocTitleError {
  #[error("a doc's title must not be blank")]
  Blank,
  #[error("a doc's title is {length} characters long; the limit is {}", Name::MAX_CHARS)]
  TooLong { length: usize },
  #[error("a doc's title is one line")]
  LineBreak,
}

/// A doc's title: one line, trimmed, not blank, at most [`Name::MAX_CHARS`] characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocTitle(Name);

impl DocTitle {
  pub fn new(value: &str) -> Result<Self, DocTitleError> {
    let name = Name::new(value).map_err(|error| match error {
      NameError::Blank => DocTitleError::Blank,
      NameError::TooLong { length } => DocTitleError::TooLong { length },
    })?;
    if name.as_str().contains(['\n', '\r']) {
      return Err(DocTitleError::LineBreak);
    }
    Ok(Self(name))
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }
}

impl fmt::Display for DocTitle {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DocSlugError {
  #[error("`{0}` is not a doc slug: lowercase letters and digits in words joined by single hyphens")]
  Invalid(String),
}

/// The part of a doc's address made from its title: lowercase letters and digits, words joined by single hyphens,
/// e.g. `Format and storage` → `format-and-storage`. Unique within its folder, which the store keeps with
/// [`DocSlug::unique_among`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocSlug(String);

impl DocSlug {
  pub const MAX_CHARS: usize = Name::MAX_CHARS;
  /// The slug of a title with no letter or digit in it.
  const FALLBACK: &str = "untitled";

  pub fn from_title(title: &DocTitle) -> Self {
    let mut slug = String::new();
    let mut length = 0;
    let mut gap = false;
    for letter in title.as_str().chars().flat_map(char::to_lowercase) {
      if !letter.is_alphanumeric() {
        gap = !slug.is_empty();
        continue;
      }
      let needed = 1 + usize::from(gap);
      if length + needed > Self::MAX_CHARS {
        break;
      }
      if gap {
        slug.push('-');
        gap = false;
      }
      slug.push(letter);
      length += needed;
    }
    if slug.is_empty() {
      return Self(Self::FALLBACK.to_owned());
    }
    Self(slug)
  }

  /// A slug as stored, checked against the slug rule.
  pub fn parse(value: &str) -> Result<Self, DocSlugError> {
    let words_valid = value
      .split('-')
      .all(|word| !word.is_empty() && word.chars().all(|c| c.is_alphanumeric() && !c.is_uppercase()));
    if words_valid && value.chars().count() <= Self::MAX_CHARS {
      Ok(Self(value.to_owned()))
    } else {
      Err(DocSlugError::Invalid(value.to_owned()))
    }
  }

  /// This slug if no sibling has it, otherwise the first of `<slug>-2`, `<slug>-3`, … that none has.
  pub fn unique_among(&self, taken: &[Self]) -> Self {
    if !taken.contains(self) {
      return self.clone();
    }
    (2..)
      .map(|number| self.numbered(number))
      .find(|candidate| !taken.contains(candidate))
      .expect("the numbers run out only after more slugs than any slice holds")
  }

  fn numbered(&self, number: u64) -> Self {
    let suffix = format!("-{number}");
    let base: String = self.0.chars().take(Self::MAX_CHARS - suffix.len()).collect();
    Self(format!("{}{suffix}", base.trim_end_matches('-')))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for DocSlug {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}
