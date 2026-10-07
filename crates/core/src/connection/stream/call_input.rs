//! What an agent CLI's call of one of Orchester's file tools asked, read from the call's input, so that its
//! transcript row reads as the native agent's row for the same tool (04.H10): what the call is about and, for
//! `edit_file` and `write_file`, the change as diff lines. The answer a CLI gets says less than the native executors
//! report (no lines read, no matches counted), so the row shows what the input asked; the call's end says whether
//! it happened. A change keeps its first [`MAX_CHANGE_LINES`] lines, each cut to [`MAX_LINE_CHARS`] characters,
//! and counts the lines it leaves out, so a stored event stays small and never shows less than it says.

use serde::{Deserialize, Serialize};

use crate::{
  diff::{LineKind, MAX_LINE_CHARS},
  native::{
    edit::shared_ends,
    tools::{Edit, ToolInput},
  },
};

/// Lines of a change a call keeps, as many as a transcript row's output block shows.
pub const MAX_CHANGE_LINES: usize = 200;

/// What a CLI's call of a file tool asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallInput {
  /// What the call is about: the file it reads, edits or writes, the folder and pattern it lists, or the pattern
  /// it searches for.
  pub target: String,
  /// The change an `edit_file` or `write_file` call asked for.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub change: Option<CallChange>,
}

/// A change as diff lines: each edit's own lines (what its `old` and `new` share, then what it removes and adds),
/// or a written file's content as added lines.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallChange {
  /// One hunk per edit, in order; a write is one hunk.
  pub hunks: Vec<Vec<ChangeLine>>,
  /// Lines left out after the last one kept.
  #[serde(default)]
  pub omitted: u64,
  /// Lines the change adds and removes: each edit's once, also one that replaces every occurrence, and every line
  /// of a written content as added (what the file held before is not known).
  pub added: u64,
  pub removed: u64,
}

/// One line of a change, without its line break.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeLine {
  pub kind: LineKind,
  pub text: String,
  /// The line was longer than [`MAX_LINE_CHARS`] and is cut there.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub cut: bool,
}

impl CallInput {
  /// What a call of a file tool with `input` asked; `None` for any other tool.
  pub fn of(input: &ToolInput) -> Option<Self> {
    let (target, change) = match input {
      ToolInput::ReadFile(read) => (read.path().to_owned(), None),
      ToolInput::ListFiles(list) => {
        let target = [list.path(), list.pattern()]
          .into_iter()
          .flatten()
          .collect::<Vec<_>>()
          .join(" ");
        (if target.is_empty() { ".".to_owned() } else { target }, None)
      }
      ToolInput::Search(search) => (search.pattern().to_owned(), None),
      ToolInput::EditFile(edit) => (edit.path().to_owned(), Some(CallChange::of_edits(edit.edits()))),
      ToolInput::WriteFile(write) => (write.path().to_owned(), Some(CallChange::of_content(write.content()))),
      ToolInput::RunCommand(_) | ToolInput::FinishStep(_) | ToolInput::AskQuestion(_) => return None,
    };
    Some(Self { target, change })
  }

  pub(super) fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    self.target = redact(&self.target);
    let lines = self
      .change
      .iter_mut()
      .flat_map(|change| change.hunks.iter_mut().flatten());
    for line in lines {
      line.text = redact(&line.text);
    }
  }
}

impl CallChange {
  fn of_edits(edits: &[Edit]) -> Self {
    let mut change = Self::default();
    for edit in edits {
      let old: Vec<&str> = edit.old().split_inclusive('\n').collect();
      let new: Vec<&str> = edit.new_text().split_inclusive('\n').collect();
      let (start, end) = shared_ends(&old, &new);
      change.added = change.added.saturating_add(count(new.len() - start - end));
      change.removed = change.removed.saturating_add(count(old.len() - start - end));
      let lines = old[..start]
        .iter()
        .map(|line| (LineKind::Context, *line))
        .chain(
          old[start..old.len() - end]
            .iter()
            .map(|line| (LineKind::Removed, *line)),
        )
        .chain(new[start..new.len() - end].iter().map(|line| (LineKind::Added, *line)))
        .chain(old[old.len() - end..].iter().map(|line| (LineKind::Context, *line)));
      change.hunk(lines);
    }
    change
  }

  fn of_content(content: &str) -> Self {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let mut change = Self {
      added: count(lines.len()),
      ..Self::default()
    };
    change.hunk(lines.into_iter().map(|line| (LineKind::Added, line)));
    change
  }

  /// Adds `lines` as a hunk, as far as the change keeps lines, and counts the others as left out.
  fn hunk<'a>(&mut self, lines: impl Iterator<Item = (LineKind, &'a str)>) {
    let room = MAX_CHANGE_LINES.saturating_sub(self.hunks.iter().map(Vec::len).sum());
    let mut hunk = Vec::new();
    for (kind, text) in lines {
      if hunk.len() < room {
        hunk.push(ChangeLine::new(kind, text));
      } else {
        self.omitted = self.omitted.saturating_add(1);
      }
    }
    if !hunk.is_empty() {
      self.hunks.push(hunk);
    }
  }
}

impl ChangeLine {
  fn new(kind: LineKind, line: &str) -> Self {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    let cut = line.chars().count() > MAX_LINE_CHARS;
    let text = if cut {
      line.chars().take(MAX_LINE_CHARS).collect()
    } else {
      line.to_owned()
    };
    Self { kind, text, cut }
  }
}

fn count(lines: usize) -> u64 {
  u64::try_from(lines).unwrap_or(u64::MAX)
}
