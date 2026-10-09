//! The change a CLI's call of Orchester's `edit_file` or `create_file` asked for, as the lines its transcript row
//! shows: each edit a hunk of its own, with what its `old` and `new` share at their start and end as context
//! (compared without their line breaks, so a line that only gains a break stays context), what it removes and what it
//! adds; a write's content as added lines. The lines are the edit's own text, not the file's: they carry no line
//! numbers. A change keeps its first [`MAX_CHANGE_LINES`] lines, each cut to [`MAX_LINE_CHARS`] characters, and counts
//! the lines it leaves out, so a stored event stays small and never shows less than it says.

use serde::{Deserialize, Serialize};

use super::call_input::{EditText, Requested};
use crate::{
  diff::{LineKind, MAX_LINE_CHARS},
  native::edit::{LineChanges, shared_ends},
};

/// Lines of a change a call keeps, as many as a transcript row's output block shows.
pub const MAX_CHANGE_LINES: usize = 200;

/// A change as hunks of lines.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallChange {
  hunks: Vec<ChangeHunk>,
  /// Lines left out after the last one kept.
  #[serde(default)]
  omitted: u64,
  /// Lines added and removed; `None` when an edit replaces every occurrence, as many as there are.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  counts: Option<LineChanges>,
}

/// The lines of one edit, or of a write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeHunk {
  lines: Vec<ChangeLine>,
  /// The edit replaces every occurrence of its `old`.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  every: bool,
  #[serde(flatten)]
  breaks: Breaks,
}

/// What an edit changes in the line breaks of the lines it keeps, which its lines do not show. Each such line counts
/// as removed and added, as git counts it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breaks {
  /// Kept lines change between CRLF and LF.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub line_endings: bool,
  /// The last line gains or loses its line break.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub final_break: Option<FinalBreak>,
}

/// Whether an edit adds the final line break or removes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalBreak {
  Added,
  Removed,
}

/// One line of a change, without its line break.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeLine {
  kind: LineKind,
  text: String,
  /// The line was longer than [`MAX_LINE_CHARS`] and is cut there.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  cut: bool,
}

/// An edit's lines, the lines it adds and removes, and what it changes in the breaks of the lines it keeps.
struct EditLines<'a> {
  lines: Vec<(LineKind, &'a str)>,
  added: u64,
  removed: u64,
  breaks: Breaks,
}

impl CallChange {
  /// The change `requested` asks for, cut to size.
  pub fn of(requested: &Requested) -> Self {
    let mut change = Self::default();
    match requested {
      Requested::Edits(edits) => {
        let mut counts = Some(LineChanges::default());
        for edit in edits {
          let lines = edit_lines(edit);
          counts = counts.filter(|_| !edit.replace_all).map(|counts| LineChanges {
            added: counts.added.saturating_add(lines.added),
            removed: counts.removed.saturating_add(lines.removed),
          });
          change.hunk(&lines.lines, edit.replace_all, lines.breaks);
        }
        change.counts = counts;
      }
      Requested::Content(content) => {
        let lines: Vec<&str> = content.split_inclusive('\n').collect();
        change.counts = Some(LineChanges {
          added: count(lines.len()),
          removed: 0,
        });
        let added: Vec<(LineKind, &str)> = lines.into_iter().map(|line| (LineKind::Added, line)).collect();
        change.hunk(&added, false, Breaks::default());
      }
    }
    change
  }

  pub fn hunks(&self) -> &[ChangeHunk] {
    &self.hunks
  }

  pub const fn omitted(&self) -> u64 {
    self.omitted
  }

  /// Lines added and removed, unless an edit replaces every occurrence.
  pub const fn counts(&self) -> Option<LineChanges> {
    self.counts
  }

  /// Whether it has no line at all: a write of an empty file.
  pub fn is_empty(&self) -> bool {
    self.hunks.is_empty() && self.omitted == 0
  }

  pub(super) fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    for line in self.hunks.iter_mut().flat_map(|hunk| hunk.lines.iter_mut()) {
      line.text = redact(&line.text);
    }
  }

  /// Adds `lines` as a hunk, as far as the change keeps lines, and counts the others as left out.
  fn hunk(&mut self, lines: &[(LineKind, &str)], every: bool, breaks: Breaks) {
    let room = MAX_CHANGE_LINES.saturating_sub(self.hunks.iter().map(|hunk| hunk.lines.len()).sum());
    let kept = lines.len().min(room);
    self.omitted = self.omitted.saturating_add(count(lines.len() - kept));
    if kept > 0 {
      self.hunks.push(ChangeHunk {
        lines: lines[..kept]
          .iter()
          .map(|(kind, text)| ChangeLine::new(*kind, text))
          .collect(),
        every,
        breaks,
      });
    }
  }
}

impl ChangeHunk {
  pub fn lines(&self) -> &[ChangeLine] {
    &self.lines
  }

  /// The edit replaces every occurrence of its `old`.
  pub const fn every(&self) -> bool {
    self.every
  }

  /// What the edit changes in the line breaks of the lines it keeps.
  pub const fn breaks(&self) -> Breaks {
    self.breaks
  }
}

impl ChangeLine {
  fn new(kind: LineKind, line: &str) -> Self {
    let line = without_break(line);
    let cut = line.chars().count() > MAX_LINE_CHARS;
    let text = if cut {
      line.chars().take(MAX_LINE_CHARS).collect()
    } else {
      line.to_owned()
    };
    Self { kind, text, cut }
  }

  pub const fn kind(&self) -> LineKind {
    self.kind
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  /// The line was longer than [`MAX_LINE_CHARS`] and is cut there.
  pub const fn cut(&self) -> bool {
    self.cut
  }
}

fn edit_lines(edit: &EditText) -> EditLines<'_> {
  let old_raw: Vec<&str> = edit.old.split_inclusive('\n').collect();
  let new_raw: Vec<&str> = edit.new.split_inclusive('\n').collect();
  let old: Vec<&str> = old_raw.iter().map(|line| without_break(line)).collect();
  let new: Vec<&str> = new_raw.iter().map(|line| without_break(line)).collect();
  let (start, end) = shared_ends(&old, &new);
  let (old_end, new_end) = (old.len() - end, new.len() - end);
  let shared = (0..start)
    .map(|at| (at, at))
    .chain((0..end).map(|at| (old_end + at, new_end + at)));
  let (breaks, changed) = changed_breaks(shared, (&old_raw, &new_raw));
  let lines = old_raw[..start]
    .iter()
    .map(|line| (LineKind::Context, *line))
    .chain(old_raw[start..old_end].iter().map(|line| (LineKind::Removed, *line)))
    .chain(new_raw[start..new_end].iter().map(|line| (LineKind::Added, *line)))
    .chain(old_raw[old_end..].iter().map(|line| (LineKind::Context, *line)))
    .collect();
  EditLines {
    lines,
    added: count(new_end - start + changed),
    removed: count(old_end - start + changed),
    breaks,
  }
}

/// What the `shared` lines (their positions in `old` and `new`) change in their breaks, and how many change: CRLF to
/// LF or back, or the last line's break added or removed. A line that gains its break because the edit adds lines
/// after it changes nothing of its own.
fn changed_breaks(shared: impl Iterator<Item = (usize, usize)>, (old, new): (&[&str], &[&str])) -> (Breaks, usize) {
  let mut breaks = Breaks::default();
  let mut changed = 0;
  for (at_old, at_new) in shared {
    let (old_line, new_line) = (old[at_old], new[at_new]);
    if old_line == new_line {
      continue;
    }
    let (old_break, new_break) = (old_line.ends_with('\n'), new_line.ends_with('\n'));
    if old_break && new_break {
      breaks.line_endings = true;
      changed += 1;
    } else if at_old + 1 == old.len() && at_new + 1 == new.len() {
      breaks.final_break = Some(if new_break {
        FinalBreak::Added
      } else {
        FinalBreak::Removed
      });
      changed += 1;
    }
  }
  (breaks, changed)
}

/// `line` without its line break, LF or CRLF.
fn without_break(line: &str) -> &str {
  let line = line.strip_suffix('\n').unwrap_or(line);
  line.strip_suffix('\r').unwrap_or(line)
}

fn count(lines: usize) -> u64 {
  u64::try_from(lines).unwrap_or(u64::MAX)
}
