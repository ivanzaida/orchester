//! What a person writes in a record editor (*New record*, *Edit*, *Review proposed record*): the type, an optional
//! key, the text, tags and importance, made into a record's content by core's rules.

use orchester_core::memory::{
  Importance, Memory, MemoryContent, MemoryContentError, MemoryKey, MemoryKind, MemoryScope, Tags,
};

use crate::messages::Message;

/// A record as an editor holds it while it is edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordDraft {
  pub kind: MemoryKind,
  /// Blank for no key.
  pub key: String,
  pub text: String,
  pub tags: Vec<String>,
  pub importance: Importance,
}

impl RecordDraft {
  /// What `memory` says, to edit.
  pub fn of(memory: &Memory) -> Self {
    let content = memory.content();
    Self {
      kind: content.kind(),
      key: content.key().map_or_else(String::new, |key| key.as_str().to_owned()),
      text: content.text().to_owned(),
      tags: content.tags().iter().map(str::to_owned).collect(),
      importance: content.importance(),
    }
  }

  /// A new record: a fact of normal importance, saying nothing yet.
  pub const fn blank() -> Self {
    Self {
      kind: MemoryKind::Fact,
      key: String::new(),
      text: String::new(),
      tags: Vec::new(),
      importance: Importance::Normal,
    }
  }

  /// The record's content, or why the draft cannot be one.
  pub fn content(&self) -> Result<MemoryContent, DraftProblem> {
    let mut content = MemoryContent::new(self.kind, &self.text).map_err(|_| DraftProblem::BlankText)?;
    if !self.key.trim().is_empty() {
      content = content.with_key(MemoryKey::new(&self.key).map_err(DraftProblem::Key)?);
    }
    let tags = Tags::new(self.tags.iter().map(String::as_str)).map_err(|_| DraftProblem::Tag)?;
    Ok(content.with_tags(tags).with_importance(self.importance))
  }

  /// Whether the draft says something other than `memory` does.
  pub fn differs_from(&self, memory: &Memory) -> bool {
    *self != Self::of(memory)
  }

  /// `tag` added, as a record keeps tags (lowercase, each once); why not when it is no tag.
  pub fn with_tag(&self, tag: &str) -> Result<Self, DraftProblem> {
    let tags = Tags::new([tag]).map_err(|_| DraftProblem::Tag)?;
    let mut draft = self.clone();
    for tag in tags.iter() {
      if !draft.tags.iter().any(|kept| kept == tag) {
        draft.tags.push(tag.to_owned());
      }
    }
    Ok(draft)
  }
}

/// Why a draft is not a record's content yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftProblem {
  BlankText,
  Key(MemoryContentError),
  Tag,
}

impl DraftProblem {
  pub fn message(&self) -> Message {
    match self {
      Self::BlankText => Message::new("memory.errors.blankText"),
      Self::Key(MemoryContentError::KeyTooLong { .. }) => {
        Message::new("memory.errors.keyTooLong").with("max", MemoryKey::MAX_CHARS.to_string())
      }
      Self::Key(_) => Message::new("memory.errors.keyOneLine"),
      Self::Tag => Message::new("memory.errors.tag").with("max", Tags::MAX_CHARS.to_string()),
    }
  }
}

/// Where a person's new record goes (*Save to*): this task's memory or the project's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SaveTo {
  Task,
  Project,
}

impl SaveTo {
  /// The scope a record saved from task `task`'s page goes to; the project's page saves to the project.
  pub const fn scope(self, task: Option<orchester_core::id::TaskId>) -> MemoryScope {
    match (self, task) {
      (Self::Task, Some(task)) => MemoryScope::Task(task),
      (Self::Task | Self::Project, _) => MemoryScope::Project,
    }
  }
}
