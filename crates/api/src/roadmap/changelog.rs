//! A release's changelog as a team server keeps it (`docs/design/roadmap.md`, decision 3): a block document in Docs'
//! model, the tasks hidden from it, and the edited entries kept aside for tasks that left. It changes under its
//! release's revision and freezes once the release is released.

use std::collections::HashSet;

use orchester_core::{
  docs::blocks::Body,
  id::{IdempotencyKey, ReleaseId, TaskId},
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ChangelogError {
  #[error("the document's Markdown has {blocks} blocks, but {ids} block ids are given")]
  BlockCount { blocks: usize, ids: usize },
  #[error("block id {0} is given twice")]
  DuplicateBlock(u64),
}

/// The changelog's document: its Markdown as core's `docs::blocks` writes it, and the id of each of its blocks in
/// order, so an edit made on one computer keeps the block ids the others focus and merge by. The ids are as many as the
/// blocks the Markdown parses into, each once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DocumentWire", into = "DocumentWire")]
pub struct ChangelogDocument {
  markdown: String,
  blocks: Vec<u64>,
}

impl ChangelogDocument {
  pub fn new(markdown: String, blocks: Vec<u64>) -> Result<Self, ChangelogError> {
    let parsed = Body::parse(&markdown).blocks().len();
    if parsed != blocks.len() {
      return Err(ChangelogError::BlockCount {
        blocks: parsed,
        ids: blocks.len(),
      });
    }
    let mut seen = HashSet::new();
    if let Some(twice) = blocks.iter().find(|id| !seen.insert(**id)) {
      return Err(ChangelogError::DuplicateBlock(*twice));
    }
    Ok(Self { markdown, blocks })
  }

  /// The document of `body`, with its blocks' ids; refused when the blocks do not round-trip through Markdown.
  pub fn of_body(body: &Body) -> Result<Self, ChangelogError> {
    let ids = body.blocks().iter().map(|block| block.id().get()).collect();
    Self::new(body.to_markdown(), ids)
  }

  /// An empty document, as a new release's changelog is.
  pub const fn empty() -> Self {
    Self {
      markdown: String::new(),
      blocks: Vec::new(),
    }
  }

  pub fn markdown(&self) -> &str {
    &self.markdown
  }

  pub fn blocks(&self) -> &[u64] {
    &self.blocks
  }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentWire {
  markdown: String,
  blocks: Vec<u64>,
}

impl TryFrom<DocumentWire> for ChangelogDocument {
  type Error = ChangelogError;

  fn try_from(wire: DocumentWire) -> Result<Self, Self::Error> {
    Self::new(wire.markdown, wire.blocks)
  }
}

impl From<ChangelogDocument> for DocumentWire {
  fn from(document: ChangelogDocument) -> Self {
    Self {
      markdown: document.markdown,
      blocks: document.blocks,
    }
  }
}

/// A task's entry a person edited, kept aside while the task is out of the release or not Done, so it comes back as
/// they left it: its list item's text as Markdown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeptEntry {
  pub task: TaskId,
  pub markdown: String,
}

/// A release's changelog: `revision` is its release's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedChangelog {
  pub release: ReleaseId,
  pub revision: TaskRevision,
  pub document: ChangelogDocument,
  /// The release's tasks whose entry a person deleted: they stay in the release.
  pub hidden: Vec<TaskId>,
  pub kept_aside: Vec<KeptEntry>,
}

/// `PATCH …/releases/{release}/changelog`: the changelog as a whole, made on the release's `revision`, which it raises.
/// A released release's changelog is frozen and refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangelogPatch {
  pub idempotency_key: IdempotencyKey,
  pub revision: TaskRevision,
  pub document: ChangelogDocument,
  pub hidden: Vec<TaskId>,
  pub kept_aside: Vec<KeptEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangelogResponse {
  pub changelog: SharedChangelog,
}
