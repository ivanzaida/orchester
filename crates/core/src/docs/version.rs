//! Committed versions of a doc: numbered from 1, each one more than the one before, immutable once built.

use std::fmt;

use thiserror::Error;

use super::{CommitRefusal, DocAuthor, DocDraft, can_commit};
use crate::{blob::ContentHash, id::DocId, time::Timestamp};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum VersionNumberError {
  #[error("a doc's versions are numbered from 1")]
  Zero,
}

/// The number of a doc's version: 1 for its first commit, one more for each commit after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocVersionNumber(u32);

impl DocVersionNumber {
  pub const FIRST: Self = Self(1);

  pub const fn new(number: u32) -> Result<Self, VersionNumberError> {
    if number == 0 {
      return Err(VersionNumberError::Zero);
    }
    Ok(Self(number))
  }

  /// The number of the version after this one; `None` past `u32::MAX`.
  pub fn next(self) -> Option<Self> {
    self.0.checked_add(1).map(Self)
  }

  pub const fn get(self) -> u32 {
    self.0
  }
}

impl fmt::Display for DocVersionNumber {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "v{}", self.0)
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CommitMessageError {
  #[error("a commit message must not be blank; leave it out instead")]
  Blank,
  #[error(
    "a commit message is {length} characters long; the limit is {}",
    CommitMessage::MAX_CHARS
  )]
  TooLong { length: usize },
}

/// What a commit says about its change: trimmed, not blank, bounded in length.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommitMessage(String);

impl CommitMessage {
  pub const MAX_CHARS: usize = 2000;

  pub fn new(value: &str) -> Result<Self, CommitMessageError> {
    let trimmed = value.trim();
    let length = trimmed.chars().count();
    if length == 0 {
      return Err(CommitMessageError::Blank);
    }
    if length > Self::MAX_CHARS {
      return Err(CommitMessageError::TooLong { length });
    }
    Ok(Self(trimmed.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// What a commit adds to a draft's body: the body's hash, an optional message, when and by whom. The store computes
/// the hash (SHA-256, as [`ContentHash`] of a blob) from the body it commits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
  pub content_hash: ContentHash,
  pub message: Option<CommitMessage>,
  pub committed_at: Timestamp,
  /// A person committing their own draft or an agent's proposal, or an agent whose profile commits without asking.
  pub committed_by: DocAuthor,
}

/// The columns of a stored version, for the store to read one back with [`DocVersion::from_stored`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredDocVersion {
  pub doc: DocId,
  pub number: DocVersionNumber,
  pub body: String,
  pub commit: Commit,
}

/// One committed version of a doc: its Markdown body as it was committed. Never changes once built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocVersion {
  doc: DocId,
  number: DocVersionNumber,
  body: String,
  commit: Commit,
}

impl DocVersion {
  /// Commits `draft` as the version after `latest`, the doc's latest version (`None` for a doc never committed), as
  /// [`can_commit`] allows.
  pub fn commit(draft: &DocDraft, latest: Option<&Self>, commit: Commit) -> Result<Self, CommitRefusal> {
    let number = can_commit(draft, latest)?;
    Ok(Self {
      doc: draft.doc(),
      number,
      body: draft.body().to_owned(),
      commit,
    })
  }

  /// A version read back from the store it was committed to.
  pub fn from_stored(stored: StoredDocVersion) -> Self {
    Self {
      doc: stored.doc,
      number: stored.number,
      body: stored.body,
      commit: stored.commit,
    }
  }

  pub const fn doc(&self) -> DocId {
    self.doc
  }

  pub const fn number(&self) -> DocVersionNumber {
    self.number
  }

  /// The doc's text, Markdown.
  pub fn body(&self) -> &str {
    &self.body
  }

  pub const fn content_hash(&self) -> &ContentHash {
    &self.commit.content_hash
  }

  pub const fn message(&self) -> Option<&CommitMessage> {
    self.commit.message.as_ref()
  }

  pub const fn committed_at(&self) -> Timestamp {
    self.commit.committed_at
  }

  pub const fn committed_by(&self) -> &DocAuthor {
    &self.commit.committed_by
  }
}
