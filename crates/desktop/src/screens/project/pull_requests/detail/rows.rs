//! The rows a changed file's diff is drawn as, one virtualised list item each: hunk headings
//! (`n32O8`), lines (`vEZ42`) and notes for what a file's diff does not show as lines (binary
//! content, a mode change, a rename without changes, lines omitted from a very large file).

use std::sync::Arc;

use orchester_core::diff::{FileChange, FileDiff, LineKind, ModeChange};

use crate::pull_requests::highlight::{FileTokens, LineTokens, plain};

/// One row of a file's diff. `index` is the row's position, its key in the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffRow {
  Hunk {
    index: usize,
    old: Arc<str>,
    new: Arc<str>,
    section: Arc<str>,
  },
  Line(Box<LineRow>),
  Note {
    index: usize,
    note: FileNote,
  },
  /// Room under the last row for the horizontal scrollbar, so it does not cover that row.
  ScrollbarGap {
    index: usize,
  },
}

impl DiffRow {
  pub const fn index(&self) -> usize {
    match self {
      Self::Hunk { index, .. } | Self::Note { index, .. } | Self::ScrollbarGap { index } => *index,
      Self::Line(line) => line.index,
    }
  }
}

/// A line of a hunk, with the width its line number columns share across the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRow {
  pub index: usize,
  pub kind: LineKind,
  pub old: Option<u32>,
  pub new: Option<u32>,
  pub tokens: LineTokens,
  /// The line was cut to its first characters.
  pub cut: bool,
  pub no_newline: bool,
  /// Digits of the file's largest line number.
  pub digits: usize,
}

/// What a file's diff says without lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileNote {
  Binary,
  Mode {
    old: Arc<str>,
    new: Arc<str>,
  },
  /// Renamed or copied without changing its content.
  Unchanged,
  /// An added or deleted file without lines.
  Empty,
  /// Lines of a very large file that are not shown.
  Omitted(u32),
}

/// The rows of `file`, its lines drawn with `tokens` when highlighting finished (one entry per
/// hunk and kept line), plain otherwise.
pub fn rows(file: &FileDiff, tokens: Option<&FileTokens>) -> Vec<DiffRow> {
  let mut rows = Vec::new();
  let mut notes = Vec::new();
  if let Some(ModeChange { old, new }) = file.mode() {
    notes.push(FileNote::Mode {
      old: old.as_str().into(),
      new: new.as_str().into(),
    });
  }
  if file.is_binary() {
    notes.push(FileNote::Binary);
  } else if file.hunks().is_empty() {
    match file.change() {
      FileChange::Renamed { .. } | FileChange::Copied { .. } => notes.push(FileNote::Unchanged),
      FileChange::Added | FileChange::Deleted => notes.push(FileNote::Empty),
      FileChange::Modified => {}
    }
  }
  for note in notes {
    let index = rows.len();
    rows.push(DiffRow::Note { index, note });
  }
  let digits = digits(file);
  for (hunk_index, hunk) in file.hunks().iter().enumerate() {
    let index = rows.len();
    rows.push(DiffRow::Hunk {
      index,
      old: range(hunk.old_start(), hunk.old_count()).into(),
      new: range(hunk.new_start(), hunk.new_count()).into(),
      section: hunk.section().into(),
    });
    let highlighted = tokens.and_then(|tokens| tokens.get(hunk_index));
    for (line_index, line) in hunk.lines().iter().enumerate() {
      let index = rows.len();
      let tokens = highlighted
        .and_then(|lines| lines.get(line_index))
        .cloned()
        .unwrap_or_else(|| plain(line.text()));
      rows.push(DiffRow::Line(Box::new(LineRow {
        index,
        kind: line.kind(),
        old: line.old_number(),
        new: line.new_number(),
        tokens,
        cut: line.is_cut(),
        no_newline: line.has_no_newline(),
        digits,
      })));
    }
  }
  if file.omitted_lines() > 0 {
    let index = rows.len();
    rows.push(DiffRow::Note {
      index,
      note: FileNote::Omitted(file.omitted_lines()),
    });
  }
  rows
}

/// `start,count` as git writes a hunk range; a count of one is left out.
fn range(start: u32, count: u32) -> String {
  if count == 1 {
    start.to_string()
  } else {
    format!("{start},{count}")
  }
}

/// Digits of the largest line number in `file`, at least two.
fn digits(file: &FileDiff) -> usize {
  let largest = file
    .hunks()
    .iter()
    .map(|hunk| (hunk.old_start() + hunk.old_count()).max(hunk.new_start() + hunk.new_count()))
    .max()
    .unwrap_or(0);
  largest.to_string().len().max(2)
}
