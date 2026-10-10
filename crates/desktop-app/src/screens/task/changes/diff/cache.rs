//! What the diff view keeps between renders, bounded: the highlighted tokens of the files shown last, by
//! each file's own content, so a file whose content did not change keeps its colors when the worktree is
//! read again; and the rows of the file on screen with the width they need, made once per content.

use std::{
  collections::VecDeque,
  hash::{DefaultHasher, Hash, Hasher},
  sync::Arc,
};

use orchester_core::diff::FileDiff;
use orchester_workspace::highlight::FileTokens;

use super::{
  line::content_width,
  rows::{DiffRow, rows},
};

/// How many files' tokens are kept: going back and forth between a few files does not highlight them
/// again.
const HIGHLIGHTS_KEPT: usize = 16;

/// A file's content: which file it is in the list (its source and path) and a hash of its hunks.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentKey {
  pub file: Arc<str>,
  hash: u64,
}

impl lurq::app::component::DevtoolsInspectable for ContentKey {}

impl ContentKey {
  pub fn of(file: Arc<str>, diff: &FileDiff) -> Self {
    let mut hasher = DefaultHasher::new();
    diff.path().hash(&mut hasher);
    diff.is_binary().hash(&mut hasher);
    diff.omitted_lines().hash(&mut hasher);
    for hunk in diff.hunks() {
      (hunk.old_start(), hunk.old_count(), hunk.new_start(), hunk.new_count()).hash(&mut hasher);
      hunk.section().hash(&mut hasher);
      for line in hunk.lines() {
        (line.kind(), line.text(), line.is_cut(), line.has_no_newline()).hash(&mut hasher);
      }
    }
    Self {
      file,
      hash: hasher.finish(),
    }
  }
}

/// The tokens of the files highlighted last, the latest at the back; `None` for a file no grammar knows.
#[derive(Default)]
pub struct Highlights(VecDeque<(ContentKey, Option<Arc<FileTokens>>)>);

impl Highlights {
  pub fn get(&mut self, key: &ContentKey) -> Option<Option<Arc<FileTokens>>> {
    let at = self.0.iter().position(|(kept, _)| kept == key)?;
    let entry = self.0.remove(at).expect("the position was just found");
    let tokens = entry.1.clone();
    self.0.push_back(entry);
    Some(tokens)
  }

  pub fn insert(&mut self, key: ContentKey, tokens: Option<Arc<FileTokens>>) {
    self.0.retain(|(kept, _)| *kept != key);
    if self.0.len() == HIGHLIGHTS_KEPT {
      self.0.pop_front();
    }
    self.0.push_back((key, tokens));
  }
}

/// The rows of the file on screen, plain or highlighted, with room for the horizontal scrollbar after
/// the last, and the width the widest needs.
#[derive(Clone)]
pub struct ShownRows {
  key: (ContentKey, bool),
  pub rows: Arc<Vec<DiffRow>>,
  pub width: f32,
}

impl ShownRows {
  /// The rows of `file` with `tokens`: `kept` while it is of the same content and highlighting.
  pub fn of(kept: Option<&Self>, key: &ContentKey, file: &FileDiff, tokens: Option<&FileTokens>) -> Self {
    let key = (key.clone(), tokens.is_some());
    if let Some(kept) = kept.filter(|kept| kept.key == key) {
      return kept.clone();
    }
    let mut rows = rows(file, tokens);
    let width = content_width(&rows);
    rows.push(DiffRow::ScrollbarGap { index: rows.len() });
    Self {
      key,
      rows: Arc::new(rows),
      width,
    }
  }
}
