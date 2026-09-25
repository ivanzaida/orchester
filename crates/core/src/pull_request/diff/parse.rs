//! The unified diff reader behind [`super::Diff::parse`]: a file starts at `diff --git`, extended
//! header lines say what happened to it (mode, rename, copy, binary), `---`/`+++` name its sides
//! and `@@` hunks follow. A carriage return ending a line is not part of its text.

use super::{
  DiffLine, DiffParseError, FileChange, FileDiff, Hunk, LineKind, MAX_FILE_LINES, MAX_LINE_CHARS, ModeChange, path,
};

#[derive(Clone, Copy)]
struct Line<'a> {
  number: usize,
  text: &'a str,
}

/// The diff's lines, read front to back.
struct Lines<'a> {
  lines: Vec<Line<'a>>,
  at: usize,
}

impl<'a> Lines<'a> {
  fn new(text: &'a str) -> Self {
    let lines = text
      .split('\n')
      .enumerate()
      .map(|(index, text)| Line {
        number: index + 1,
        text: text.strip_suffix('\r').unwrap_or(text),
      })
      .collect();
    Self { lines, at: 0 }
  }

  fn peek(&self) -> Option<Line<'a>> {
    self.lines.get(self.at).copied()
  }

  fn next(&mut self) -> Option<Line<'a>> {
    let line = self.peek()?;
    self.at += 1;
    Some(line)
  }

  /// The next line if `accept` takes it.
  fn next_if(&mut self, accept: impl Fn(&str) -> bool) -> Option<Line<'a>> {
    self.peek().filter(|line| accept(line.text)).and_then(|_| self.next())
  }
}

pub fn parse(text: &str) -> Result<Vec<FileDiff>, DiffParseError> {
  let text = text.strip_suffix('\n').unwrap_or(text);
  if text.is_empty() {
    return Ok(Vec::new());
  }
  let mut lines = Lines::new(text);
  let mut files = Vec::new();
  while let Some(start) = lines.next() {
    let Some(header) = start.text.strip_prefix("diff --git ") else {
      return Err(DiffParseError::UnexpectedLine { line: start.number });
    };
    files.push(file(&mut lines, start.number, header)?);
  }
  Ok(files)
}

/// What the lines of one file say so far.
struct Building {
  old_path: Option<String>,
  new_path: Option<String>,
  change: FileChange,
  old_mode: Option<String>,
  new_mode: Option<String>,
  binary: bool,
  hunks: Vec<Hunk>,
  counts: Counts,
}

#[derive(Default)]
struct Counts {
  additions: u32,
  deletions: u32,
  kept: usize,
  omitted: u32,
}

fn file(lines: &mut Lines<'_>, number: usize, header: &str) -> Result<FileDiff, DiffParseError> {
  let (old, new) = path::header(header).ok_or(DiffParseError::Path { line: number })?;
  let mut file = Building {
    old_path: Some(old),
    new_path: Some(new),
    change: FileChange::Modified,
    old_mode: None,
    new_mode: None,
    binary: false,
    hunks: Vec::new(),
    counts: Counts::default(),
  };
  while let Some(next) = lines.peek() {
    if next.text.starts_with("diff --git ") {
      break;
    }
    lines.next();
    if let Some(range) = next.text.strip_prefix("@@ ") {
      let hunk = hunk(lines, next.number, range, &mut file.counts)?;
      file.hunks.extend(hunk);
    } else if next.text == "GIT binary patch" {
      file.binary = true;
      skip_to_next_file(lines);
    } else {
      extended_header(&mut file, next)?;
    }
  }
  Ok(file.finish())
}

impl Building {
  fn finish(self) -> FileDiff {
    let mode = match (self.old_mode, self.new_mode) {
      (Some(old), Some(new)) if old != new => Some(ModeChange { old, new }),
      _ => None,
    };
    FileDiff {
      old_path: self.old_path,
      new_path: self.new_path,
      change: self.change,
      mode,
      binary: self.binary,
      hunks: self.hunks,
      additions: self.counts.additions,
      deletions: self.counts.deletions,
      omitted_lines: self.counts.omitted,
    }
  }
}

/// A line between `diff --git` and the first hunk.
fn extended_header(file: &mut Building, line: Line<'_>) -> Result<(), DiffParseError> {
  let text = line.text;
  let bad_path = || DiffParseError::Path { line: line.number };
  if let Some(mode) = text.strip_prefix("old mode ") {
    file.old_mode = Some(mode.to_owned());
  } else if let Some(mode) = text.strip_prefix("new mode ") {
    file.new_mode = Some(mode.to_owned());
  } else if text.starts_with("new file mode ") {
    file.change = FileChange::Added;
    file.old_path = None;
  } else if text.starts_with("deleted file mode ") {
    file.change = FileChange::Deleted;
    file.new_path = None;
  } else if let Some(percent) = text.strip_prefix("similarity index ") {
    let similarity = percent.strip_suffix('%').and_then(|value| value.parse().ok());
    file.change = match file.change {
      FileChange::Copied { .. } => FileChange::Copied { similarity },
      _ => FileChange::Renamed { similarity },
    };
  } else if let Some(raw) = text.strip_prefix("rename from ") {
    file.old_path = Some(path::plain(raw).ok_or_else(bad_path)?);
    file.change = renamed(&file.change);
  } else if let Some(raw) = text.strip_prefix("rename to ") {
    file.new_path = Some(path::plain(raw).ok_or_else(bad_path)?);
    file.change = renamed(&file.change);
  } else if let Some(raw) = text.strip_prefix("copy from ") {
    file.old_path = Some(path::plain(raw).ok_or_else(bad_path)?);
    file.change = copied(&file.change);
  } else if let Some(raw) = text.strip_prefix("copy to ") {
    file.new_path = Some(path::plain(raw).ok_or_else(bad_path)?);
    file.change = copied(&file.change);
  } else if let Some(raw) = text.strip_prefix("--- ") {
    if file.change != FileChange::Added {
      file.old_path = path::side(raw).ok_or_else(bad_path)?.or(file.old_path.take());
    }
  } else if let Some(raw) = text.strip_prefix("+++ ") {
    if file.change != FileChange::Deleted {
      file.new_path = path::side(raw).ok_or_else(bad_path)?.or(file.new_path.take());
    }
  } else if text.starts_with("Binary files ") && text.ends_with(" differ") {
    file.binary = true;
  } else if !(text.starts_with("index ") || text.starts_with("dissimilarity index ")) {
    return Err(DiffParseError::UnexpectedLine { line: line.number });
  }
  Ok(())
}

const fn renamed(change: &FileChange) -> FileChange {
  match change {
    FileChange::Renamed { similarity } => FileChange::Renamed {
      similarity: *similarity,
    },
    _ => FileChange::Renamed { similarity: None },
  }
}

const fn copied(change: &FileChange) -> FileChange {
  match change {
    FileChange::Copied { similarity } | FileChange::Renamed { similarity } => FileChange::Copied {
      similarity: *similarity,
    },
    _ => FileChange::Copied { similarity: None },
  }
}

fn skip_to_next_file(lines: &mut Lines<'_>) {
  while lines.next_if(|text| !text.starts_with("diff --git ")).is_some() {}
}

/// Where a hunk is: `-old_start[,old_count] +new_start[,new_count] @@ section`.
struct Range {
  old_start: u32,
  old_count: u32,
  new_start: u32,
  new_count: u32,
  section: String,
}

fn range(text: &str) -> Option<Range> {
  let (ranges, section) = text.split_once(" @@")?;
  let (old, new) = ranges.split_once(' ')?;
  let side = |value: &str| -> Option<(u32, u32)> {
    match value.split_once(',') {
      Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
      None => Some((value.parse().ok()?, 1)),
    }
  };
  let (old_start, old_count) = side(old.strip_prefix('-')?)?;
  let (new_start, new_count) = side(new.strip_prefix('+')?)?;
  Some(Range {
    old_start,
    old_count,
    new_start,
    new_count,
    section: section.strip_prefix(' ').unwrap_or(section).to_owned(),
  })
}

/// The lines of the hunk whose header is on `number`. `None` when the file has already kept
/// [`MAX_FILE_LINES`] lines: the hunk's lines are counted, not kept.
fn hunk(lines: &mut Lines<'_>, number: usize, text: &str, counts: &mut Counts) -> Result<Option<Hunk>, DiffParseError> {
  let range = range(text).ok_or(DiffParseError::HunkHeader { line: number })?;
  let (mut old_left, mut new_left) = (range.old_count, range.new_count);
  let (mut old, mut new) = (range.old_start, range.new_start);
  let mut kept: Vec<DiffLine> = Vec::new();
  let starts_full = counts.kept >= MAX_FILE_LINES;
  while old_left > 0 || new_left > 0 {
    let next = lines.next().ok_or(DiffParseError::ShortHunk { line: number })?;
    let (kind, content) = match next.text.split_at_checked(1) {
      Some(("\\", _)) => {
        mark_no_newline(&mut kept);
        continue;
      }
      Some((" ", content)) => (LineKind::Context, content),
      Some(("-", content)) => (LineKind::Removed, content),
      Some(("+", content)) => (LineKind::Added, content),
      // A context line whose single space an editor stripped.
      None => (LineKind::Context, ""),
      Some(_) => return Err(DiffParseError::UnexpectedLine { line: next.number }),
    };
    let (old_number, new_number) = match kind {
      LineKind::Context if old_left > 0 && new_left > 0 => {
        (old_left, new_left) = (old_left - 1, new_left - 1);
        (Some(old), Some(new))
      }
      LineKind::Removed if old_left > 0 => {
        old_left -= 1;
        counts.deletions += 1;
        (Some(old), None)
      }
      LineKind::Added if new_left > 0 => {
        new_left -= 1;
        counts.additions += 1;
        (None, Some(new))
      }
      _ => return Err(DiffParseError::UnexpectedLine { line: next.number }),
    };
    old += u32::from(old_number.is_some());
    new += u32::from(new_number.is_some());
    if counts.kept < MAX_FILE_LINES {
      counts.kept += 1;
      kept.push(diff_line(kind, old_number, new_number, content));
    } else {
      counts.omitted += 1;
    }
  }
  if lines.next_if(|text| text.starts_with('\\')).is_some() {
    mark_no_newline(&mut kept);
  }
  if starts_full {
    return Ok(None);
  }
  Ok(Some(Hunk {
    old_start: range.old_start,
    old_count: range.old_count,
    new_start: range.new_start,
    new_count: range.new_count,
    section: range.section,
    lines: kept,
  }))
}

fn mark_no_newline(lines: &mut [DiffLine]) {
  if let Some(last) = lines.last_mut() {
    last.no_newline = true;
  }
}

fn diff_line(kind: LineKind, old: Option<u32>, new: Option<u32>, content: &str) -> DiffLine {
  let (text, cut) = match content.char_indices().nth(MAX_LINE_CHARS) {
    Some((end, _)) => (content[..end].to_owned(), true),
    None => (content.to_owned(), false),
  };
  DiffLine {
    kind,
    old,
    new,
    text,
    cut,
    no_newline: false,
  }
}
