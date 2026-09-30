//! What a memory record says: its kind, an optional key, the text, tags, and how important and
//! how certain it is. Validated here; whether the text may be stored at all (no secret value in
//! it) is decided by [`super::SecretScreen`].

use std::collections::BTreeSet;

use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MemoryContentError {
  #[error("a memory text must not be blank")]
  BlankText,
  #[error("a memory key must not be blank")]
  BlankKey,
  #[error("a memory key is {length} characters long; the limit is {}", MemoryKey::MAX_CHARS)]
  KeyTooLong { length: usize },
  #[error("a memory key is one line")]
  MultilineKey,
  #[error("a tag is one word without spaces, at most {} characters", Tags::MAX_CHARS)]
  InvalidTag,
  #[error("importance is from 1 to 5, not {0}")]
  Importance(u8),
  #[error("confidence is a percentage from 0 to 100, not {0}")]
  Confidence(u8),
}

/// What kind of knowledge a record holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemoryKind {
  Fact,
  Decision,
  Observation,
  Artifact,
  /// What a finished step leaves the agents after it (the handoff of its result).
  Handoff,
  Todo,
  Warning,
  Summary,
}

impl MemoryKind {
  pub const ALL: [Self; 8] = [
    Self::Fact,
    Self::Decision,
    Self::Observation,
    Self::Artifact,
    Self::Handoff,
    Self::Todo,
    Self::Warning,
    Self::Summary,
  ];

  /// Stable name, as stored.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Fact => "fact",
      Self::Decision => "decision",
      Self::Observation => "observation",
      Self::Artifact => "artifact",
      Self::Handoff => "handoff",
      Self::Todo => "todo",
      Self::Warning => "warning",
      Self::Summary => "summary",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|kind| kind.name() == name)
  }

  /// How an agent's prompt labels a record of this kind.
  pub const fn label(self) -> &'static str {
    match self {
      Self::Fact => "Fact",
      Self::Decision => "Decision",
      Self::Observation => "Observation",
      Self::Artifact => "Artifact",
      Self::Handoff => "Handoff",
      Self::Todo => "To do",
      Self::Warning => "Warning",
      Self::Summary => "Summary",
    }
  }
}

/// A short name a record is known by (`auth.token-refresh`), so a newer record on the same subject
/// replaces it instead of standing beside it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemoryKey(String);

impl MemoryKey {
  pub const MAX_CHARS: usize = 120;

  /// Surrounding whitespace is not part of the key.
  pub fn new(value: &str) -> Result<Self, MemoryContentError> {
    let trimmed = value.trim();
    let length = trimmed.chars().count();
    if length == 0 {
      return Err(MemoryContentError::BlankKey);
    }
    if length > Self::MAX_CHARS {
      return Err(MemoryContentError::KeyTooLong { length });
    }
    if trimmed.contains(['\n', '\r']) {
      return Err(MemoryContentError::MultilineKey);
    }
    Ok(Self(trimmed.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A record's tags: lowercase words without spaces, each once, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tags(BTreeSet<String>);

impl Tags {
  pub const MAX_CHARS: usize = 40;

  pub fn new<'a>(tags: impl IntoIterator<Item = &'a str>) -> Result<Self, MemoryContentError> {
    let mut set = BTreeSet::new();
    for tag in tags {
      let tag = tag.trim().to_lowercase();
      if tag.is_empty() || tag.chars().count() > Self::MAX_CHARS || tag.contains(char::is_whitespace) {
        return Err(MemoryContentError::InvalidTag);
      }
      set.insert(tag);
    }
    Ok(Self(set))
  }

  pub fn iter(&self) -> impl Iterator<Item = &str> {
    self.0.iter().map(String::as_str)
  }

  pub fn contains(&self, tag: &str) -> bool {
    self.0.contains(tag)
  }

  /// The tags separated by spaces, as stored and indexed.
  pub fn joined(&self) -> String {
    self.iter().collect::<Vec<_>>().join(" ")
  }
}

/// How much a record matters, from 1 (minor) to 5 (critical).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Importance(u8);

impl Importance {
  pub const NORMAL: Self = Self(3);
  pub const HIGH: Self = Self(4);

  pub const fn new(value: u8) -> Result<Self, MemoryContentError> {
    if value >= 1 && value <= 5 {
      Ok(Self(value))
    } else {
      Err(MemoryContentError::Importance(value))
    }
  }

  pub const fn get(self) -> u8 {
    self.0
  }
}

/// How sure the author is, in percent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Confidence(u8);

impl Confidence {
  /// What a person states.
  pub const CERTAIN: Self = Self(100);
  /// What an agent reports about its own work: taken as said, not checked.
  pub const REPORTED: Self = Self(80);

  pub const fn new(percent: u8) -> Result<Self, MemoryContentError> {
    if percent <= 100 {
      Ok(Self(percent))
    } else {
      Err(MemoryContentError::Confidence(percent))
    }
  }

  pub const fn percent(self) -> u8 {
    self.0
  }
}

/// Everything a record says, validated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryContent {
  kind: MemoryKind,
  key: Option<MemoryKey>,
  text: String,
  tags: Tags,
  importance: Importance,
  confidence: Confidence,
}

impl MemoryContent {
  /// A record of `kind` saying `text` (trimmed, not blank), of normal importance and certain, with
  /// no key and no tags.
  pub fn new(kind: MemoryKind, text: &str) -> Result<Self, MemoryContentError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(MemoryContentError::BlankText);
    }
    Ok(Self {
      kind,
      key: None,
      text: text.to_owned(),
      tags: Tags::default(),
      importance: Importance::NORMAL,
      confidence: Confidence::CERTAIN,
    })
  }

  #[must_use]
  pub fn with_key(mut self, key: MemoryKey) -> Self {
    self.key = Some(key);
    self
  }

  #[must_use]
  pub fn with_tags(mut self, tags: Tags) -> Self {
    self.tags = tags;
    self
  }

  #[must_use]
  pub const fn with_importance(mut self, importance: Importance) -> Self {
    self.importance = importance;
    self
  }

  #[must_use]
  pub const fn with_confidence(mut self, confidence: Confidence) -> Self {
    self.confidence = confidence;
    self
  }

  pub const fn kind(&self) -> MemoryKind {
    self.kind
  }

  pub const fn key(&self) -> Option<&MemoryKey> {
    self.key.as_ref()
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  pub const fn tags(&self) -> &Tags {
    &self.tags
  }

  pub const fn importance(&self) -> Importance {
    self.importance
  }

  pub const fn confidence(&self) -> Confidence {
    self.confidence
  }
}
