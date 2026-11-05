//! The rows the run's whole diff is drawn as, one virtualised list item each (`Diff` `l7sLk` in 09i): every file's
//! head (`File head` `HEoFr`), then, unless the file is folded, its blocks after its head, its hunks (`Hunk` `TvvGo`),
//! lines (`Line 41` `Wd74X`) with the blocks under them, and notes for what its diff does not show as lines; room for
//! the horizontal scrollbar after the last. Made with the existing diff's rows of each file
//! (`changes::diff::rows`), so hunks, line numbers, tokens and notes read the same as on the task page.

use std::{collections::HashMap, sync::Arc};

use orchester_core::diff::{Diff, FileDiff};
use orchester_workspace::highlight::FileTokens;

use super::place::FilePlaces;
use crate::screens::task::changes::diff::rows::{DiffRow, FileNote, LineRow, rows as file_rows};

/// Advance of one Geist Mono character at 12: 0.6 em.
const CODE_CHAR_WIDTH: f32 = 7.2;
/// The line numbers, the sign and the room after the code (`Gutter` `lz3Ni`, `Sign` `Rp1hB`).
const LINE_START: f32 = 76.0 + 18.0;
const LINE_END: f32 = 24.0;

/// One row of the run's diff. `index` is the row's position, its key in the list; `file`, its file's position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::merges::review) enum ReviewRow {
  Head {
    index: usize,
    file: usize,
  },
  Hunk {
    index: usize,
    old: Arc<str>,
    new: Arc<str>,
    section: Arc<str>,
  },
  Line {
    index: usize,
    file: usize,
    line: Box<LineRow>,
  },
  Note {
    index: usize,
    note: FileNote,
  },
  /// A thread, a fold or the comment box, which the page draws by its key.
  Block {
    index: usize,
    key: Arc<str>,
  },
  /// Room under the last row for the horizontal scrollbar.
  End {
    index: usize,
  },
}

impl ReviewRow {
  pub const fn index(&self) -> usize {
    match self {
      Self::Head { index, .. }
      | Self::Hunk { index, .. }
      | Self::Line { index, .. }
      | Self::Note { index, .. }
      | Self::Block { index, .. }
      | Self::End { index } => *index,
    }
  }
}

/// The rows of `diff` and the width the widest line needs.
pub(super) struct Made {
  pub rows: Arc<Vec<ReviewRow>>,
  pub width: f32,
}

/// The rows of every file of `diff`, those that `folded` names only as their heads; each file's lines drawn with its
/// `tokens` once highlighted, plain before, and what it shows besides them (`places`).
pub(super) fn made(
  diff: &Diff,
  tokens: Option<&[Option<Arc<FileTokens>>]>,
  places: &HashMap<Arc<str>, FilePlaces>,
  folded: &dyn Fn(&FileDiff) -> bool,
) -> Made {
  let none = FilePlaces::default();
  let mut rows = Vec::new();
  let mut width: f32 = 0.0;
  for (at, file) in diff.files().iter().enumerate() {
    rows.push(ReviewRow::Head {
      index: rows.len(),
      file: at,
    });
    if folded(file) {
      continue;
    }
    let place = places.get(file.path()).unwrap_or(&none);
    for key in &place.top {
      rows.push(ReviewRow::Block {
        index: rows.len(),
        key: key.clone(),
      });
    }
    let file_tokens = tokens.and_then(|tokens| tokens.get(at)).and_then(Option::as_deref);
    for row in file_rows(file, file_tokens, Some(place.marks())) {
      let index = rows.len();
      rows.push(match row {
        DiffRow::Hunk { old, new, section, .. } => ReviewRow::Hunk {
          index,
          old,
          new,
          section,
        },
        DiffRow::Line(mut line) => {
          width = width.max(line_width(&line));
          line.index = index;
          ReviewRow::Line { index, file: at, line }
        }
        DiffRow::Note { note, .. } => ReviewRow::Note { index, note },
        DiffRow::Block { key, .. } => ReviewRow::Block { index, key },
        DiffRow::ScrollbarGap { .. } => continue,
      });
    }
  }
  rows.push(ReviewRow::End { index: rows.len() });
  Made {
    rows: Arc::new(rows),
    width,
  }
}

#[allow(
  clippy::cast_precision_loss,
  reason = "a line holds at most `MAX_LINE_CHARS` characters, far inside f32's exact range"
)]
fn line_width(line: &LineRow) -> f32 {
  let chars: usize = line.tokens.iter().map(|token| token.text.chars().count()).sum();
  LINE_START + chars as f32 * CODE_CHAR_WIDTH + LINE_END
}
