//! The rows a changed file's diff is drawn as, one virtualised list item each: hunk headings (`Diff hunk`
//! `sphVU`), lines (`Diff line/context` `L5WYtA`, `/remove` `tWgTT`, `/add` `C8FPa`) and notes for what a
//! file's diff does not show as lines (binary content, a mode change, a rename without changes, lines
//! left out of a very large file); and what a page draws into it ([`super::annotate`]): marked lines and
//! blocks under the line they follow.

use std::sync::Arc;

use orchester_core::diff::{FileChange, FileDiff, LineKind, ModeChange};
use orchester_workspace::highlight::{FileTokens, LineTokens, plain};

use super::annotate::{LineKey, Marks};

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
  /// A page's block under a line, which the page draws by its key.
  Block {
    index: usize,
    key: Arc<str>,
  },
  /// Room under the last row for the horizontal scrollbar, so it does not cover that row.
  ScrollbarGap {
    index: usize,
  },
}

impl DiffRow {
  pub const fn index(&self) -> usize {
    match self {
      Self::Hunk { index, .. }
      | Self::Note { index, .. }
      | Self::Block { index, .. }
      | Self::ScrollbarGap { index } => *index,
      Self::Line(line) => line.index,
    }
  }
}

/// A line of a hunk.
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
  /// A page marked it as chosen.
  pub marked: bool,
}

impl LineRow {
  pub fn key(&self) -> Option<LineKey> {
    LineKey::of_numbers(self.kind, (self.old, self.new))
  }
}

/// What a file's diff says without lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileNote {
  Binary,
  Mode {
    old: Arc<str>,
    new: Arc<str>,
  },
  /// Renamed or copied from `from` without changing its content.
  Unchanged {
    from: Arc<str>,
  },
  /// An added or deleted file without lines.
  Empty,
  /// Lines of a very large file that are not shown.
  Omitted(u32),
}

/// The rows of `file`, its lines drawn with `tokens` when highlighting finished (one entry per hunk and
/// kept line), plain otherwise; with `marks`, a page's marked lines and its blocks under their lines.
pub fn rows(file: &FileDiff, tokens: Option<&FileTokens>, marks: Option<Marks<'_>>) -> Vec<DiffRow> {
  let mut rows: Vec<DiffRow> = notes(file)
    .into_iter()
    .enumerate()
    .map(|(index, note)| DiffRow::Note { index, note })
    .collect();
  for (hunk_index, hunk) in file.hunks().iter().enumerate() {
    rows.push(DiffRow::Hunk {
      index: rows.len(),
      old: range(hunk.old_start(), hunk.old_count()).into(),
      new: range(hunk.new_start(), hunk.new_count()).into(),
      section: hunk.section().into(),
    });
    let highlighted = tokens.and_then(|tokens| tokens.get(hunk_index));
    for (line_index, line) in hunk.lines().iter().enumerate() {
      let tokens = highlighted
        .and_then(|lines| lines.get(line_index))
        .cloned()
        .unwrap_or_else(|| plain(line.text()));
      let key = LineKey::of_numbers(line.kind(), (line.old_number(), line.new_number()));
      rows.push(DiffRow::Line(Box::new(LineRow {
        index: rows.len(),
        kind: line.kind(),
        old: line.old_number(),
        new: line.new_number(),
        tokens,
        cut: line.is_cut(),
        no_newline: line.has_no_newline(),
        marked: marks.is_some_and(|marks| key.is_some_and(|key| marks.marked.contains(&key))),
      })));
      let blocks = marks
        .into_iter()
        .flat_map(|marks| marks.blocks.iter())
        .filter(|(under, _)| Some(*under) == key);
      for (_, block) in blocks {
        rows.push(DiffRow::Block {
          index: rows.len(),
          key: block.clone(),
        });
      }
    }
  }
  if file.omitted_lines() > 0 {
    rows.push(DiffRow::Note {
      index: rows.len(),
      note: FileNote::Omitted(file.omitted_lines()),
    });
  }
  rows
}

/// What `file`'s diff says before its lines.
fn notes(file: &FileDiff) -> Vec<FileNote> {
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
      FileChange::Renamed { .. } | FileChange::Copied { .. } => notes.push(FileNote::Unchanged {
        from: file.old_path().unwrap_or_default().into(),
      }),
      FileChange::Added | FileChange::Deleted => notes.push(FileNote::Empty),
      FileChange::Modified => {}
    }
  }
  notes
}

/// `start,count` as git writes a hunk range; a count of one is left out.
fn range(start: u32, count: u32) -> String {
  if count == 1 {
    start.to_string()
  } else {
    format!("{start},{count}")
  }
}
