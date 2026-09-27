//! Changes as a unified diff (`git diff` format, as git and GitHub write it): files, their hunks
//! and lines with old and new line numbers. Built only by [`Diff::parse`]. Very large files keep
//! their first [`MAX_FILE_LINES`] lines and count the rest as omitted; very long lines keep their
//! first [`MAX_LINE_CHARS`] characters and are marked as cut, so a diff never shows less than it
//! claims without saying so. Pull requests and runs show their changes with it.

mod parse;
mod path;
mod size;

use thiserror::Error;

pub use self::size::{Changes, MAX_DIFF_FILES, MAX_DIFF_LINES};
use crate::{revision::Revision, time::Timestamp};

/// Lines of one file a diff keeps; later lines are counted in [`FileDiff::omitted_lines`].
pub const MAX_FILE_LINES: usize = 5_000;
/// Characters of one line a diff keeps; a longer line is [`DiffLine::is_cut`].
pub const MAX_LINE_CHARS: usize = 1_000;

/// The unified diff from `base` to `head`, read at `read_at`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevisionDiff {
  pub base: Revision,
  pub head: Revision,
  pub text: String,
  pub read_at: Timestamp,
}

/// Why a text is not a unified diff. `line` counts from 1.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DiffParseError {
  #[error("line {line} is not part of a file's diff")]
  UnexpectedLine { line: usize },
  #[error("line {line} is not a hunk header")]
  HunkHeader { line: usize },
  #[error("the hunk starting on line {line} ends before its lines do")]
  ShortHunk { line: usize },
  #[error("line {line} names no file")]
  Path { line: usize },
}

/// All changed files of a diff, in the order the diff lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
  files: Vec<FileDiff>,
}

impl Diff {
  /// Reads `text`, the output of `git diff` or GitHub's diff media type. An empty text is a diff
  /// without files.
  pub fn parse(text: &str) -> Result<Self, DiffParseError> {
    parse::parse(text).map(|files| Self { files })
  }

  pub fn files(&self) -> &[FileDiff] {
    &self.files
  }

  /// Added lines of all files, omitted ones included.
  pub fn additions(&self) -> u32 {
    self.files.iter().map(|file| file.additions).sum()
  }

  /// Removed lines of all files, omitted ones included.
  pub fn deletions(&self) -> u32 {
    self.files.iter().map(|file| file.deletions).sum()
  }
}

/// What happened to a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileChange {
  Added,
  Deleted,
  Modified,
  /// Moved from [`FileDiff::old_path`], possibly with changes; `similarity` in percent.
  Renamed {
    similarity: Option<u8>,
  },
  /// Copied from [`FileDiff::old_path`].
  Copied {
    similarity: Option<u8>,
  },
}

/// A file's permission bits before and after, e.g. `100644` → `100755`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeChange {
  pub old: String,
  pub new: String,
}

/// One changed file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDiff {
  old_path: Option<String>,
  new_path: Option<String>,
  change: FileChange,
  mode: Option<ModeChange>,
  binary: bool,
  hunks: Vec<Hunk>,
  additions: u32,
  deletions: u32,
  omitted_lines: u32,
}

impl FileDiff {
  /// The path before the change; `None` for an added file.
  pub fn old_path(&self) -> Option<&str> {
    self.old_path.as_deref()
  }

  /// The path after the change; `None` for a deleted file.
  pub fn new_path(&self) -> Option<&str> {
    self.new_path.as_deref()
  }

  /// The path the file has after the change, or had before it was deleted.
  pub fn path(&self) -> &str {
    self
      .new_path
      .as_deref()
      .or(self.old_path.as_deref())
      .expect("the parser gives every file at least one path")
  }

  pub const fn change(&self) -> &FileChange {
    &self.change
  }

  pub const fn mode(&self) -> Option<&ModeChange> {
    self.mode.as_ref()
  }

  /// Binary content: the diff says the file changed, not how.
  pub const fn is_binary(&self) -> bool {
    self.binary
  }

  pub fn hunks(&self) -> &[Hunk] {
    &self.hunks
  }

  pub const fn additions(&self) -> u32 {
    self.additions
  }

  pub const fn deletions(&self) -> u32 {
    self.deletions
  }

  /// Lines beyond [`MAX_FILE_LINES`] that the diff counted but did not keep.
  pub const fn omitted_lines(&self) -> u32 {
    self.omitted_lines
  }
}

/// A run of changed lines with context, `@@ -old_start,old_count +new_start,new_count @@`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
  old_start: u32,
  old_count: u32,
  new_start: u32,
  new_count: u32,
  section: String,
  lines: Vec<DiffLine>,
}

impl Hunk {
  pub const fn old_start(&self) -> u32 {
    self.old_start
  }

  pub const fn old_count(&self) -> u32 {
    self.old_count
  }

  pub const fn new_start(&self) -> u32 {
    self.new_start
  }

  pub const fn new_count(&self) -> u32 {
    self.new_count
  }

  /// The enclosing code git names after the range (e.g. a function signature); may be empty.
  pub fn section(&self) -> &str {
    &self.section
  }

  /// The kept lines; fewer than the counts when the file reached [`MAX_FILE_LINES`].
  pub fn lines(&self) -> &[DiffLine] {
    &self.lines
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
  Context,
  Added,
  Removed,
}

/// One line of a hunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
  kind: LineKind,
  old: Option<u32>,
  new: Option<u32>,
  text: String,
  cut: bool,
  no_newline: bool,
}

impl DiffLine {
  pub const fn kind(&self) -> LineKind {
    self.kind
  }

  /// Line number before the change; `None` for an added line.
  pub const fn old_number(&self) -> Option<u32> {
    self.old
  }

  /// Line number after the change; `None` for a removed line.
  pub const fn new_number(&self) -> Option<u32> {
    self.new
  }

  /// The line without its `+`, `-` or space marker and without a trailing carriage return.
  pub fn text(&self) -> &str {
    &self.text
  }

  /// The line was longer than [`MAX_LINE_CHARS`] and [`Self::text`] is its beginning.
  pub const fn is_cut(&self) -> bool {
    self.cut
  }

  /// The file ends with this line and without a newline ("\ No newline at end of file").
  pub const fn has_no_newline(&self) -> bool {
    self.no_newline
  }
}
