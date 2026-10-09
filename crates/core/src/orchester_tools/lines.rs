//! The text change of an `edit_file` call: lines `start_line..=end_line` of the file become the call's content,
//! only when they are exactly its `expected` lines now; otherwise the call is refused with what those lines hold now.
//! Line endings and a byte order mark are the file's: lines compare without their ending, a file whose every line ends
//! in CRLF gets CRLF on the new lines too, and a last line without an ending keeps having none.

use std::fmt::Write;

use thiserror::Error;

use super::files::LineEdit;
use crate::native::edit::{Edited, LineChanges};

const BOM: char = '\u{feff}';
/// The most lines a refusal shows of what the range holds now.
const MAX_SHOWN_LINES: usize = 200;
/// The most bytes a refusal shows of what the range holds now.
const MAX_SHOWN_BYTES: usize = 16 * 1024;

/// Why an edit is not made. The message is the error result the agent gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LineEditError {
  #[error("the file has {lines} lines: lines {start}–{end} are not all in it; read it again")]
  PastEnd { lines: usize, start: u32, end: u32 },
  #[error("lines {start}–{end} are not what `expected` says; nothing was changed. They are now:\n{now}")]
  Changed { start: u32, end: u32, now: String },
}

/// `text` with `edit` made, and the lines it changed.
pub fn replace_lines(text: &str, edit: &LineEdit) -> Result<Edited, LineEditError> {
  let (bom, body) = match text.strip_prefix(BOM) {
    Some(body) => (true, body),
    None => (false, text),
  };
  let lines: Vec<&str> = body.split_inclusive('\n').collect();
  let (start, end) = (edit.start_line(), edit.end_line());
  let from = index(start) - 1;
  let to = index(end);
  if to > lines.len() {
    return Err(LineEditError::PastEnd {
      lines: lines.len(),
      start,
      end,
    });
  }
  let current = &lines[from..to];
  if !same(current, edit.expected()) {
    return Err(LineEditError::Changed {
      start,
      end,
      now: numbered(current, start),
    });
  }
  let ending = if only_crlf(body) { "\r\n" } else { "\n" };
  // A last line without an ending keeps having none, also when lines are added after it.
  let open_end = to == lines.len() && lines.last().is_some_and(|last| !last.ends_with('\n'));
  let new_lines = content_lines(edit.content());
  let mut changed = String::with_capacity(body.len() + edit.content().len());
  if bom {
    changed.push(BOM);
  }
  lines[..from].iter().for_each(|line| changed.push_str(line));
  if open_end && from == lines.len() && !new_lines.is_empty() {
    changed.push_str(ending);
  }
  for (at, line) in new_lines.iter().enumerate() {
    changed.push_str(line);
    if !(open_end && at + 1 == new_lines.len()) {
      changed.push_str(ending);
    }
  }
  lines[to..].iter().for_each(|line| changed.push_str(line));
  Ok(Edited {
    text: changed,
    lines: LineChanges::between(&joined(&content_lines(edit.expected())), &joined(&new_lines)),
  })
}

/// What lines `start..=end` of `text` hold now, numbered, as a refusal shows them; the lines past the file's end are
/// left out.
pub fn range_now(text: &str, start: u32, end: u32) -> String {
  let body = text.strip_prefix(BOM).unwrap_or(text);
  let lines: Vec<&str> = body.split_inclusive('\n').collect();
  let from = (index(start) - 1).min(lines.len());
  let to = index(end).clamp(from, lines.len());
  numbered(&lines[from..to], start)
}

fn index(line: u32) -> usize {
  usize::try_from(line).unwrap_or(usize::MAX)
}

/// Whether `current` (lines with their endings) are exactly the lines of `expected`.
fn same(current: &[&str], expected: &str) -> bool {
  let expected = content_lines(expected);
  current.len() == expected.len()
    && current
      .iter()
      .zip(&expected)
      .all(|(line, wanted)| bare(line) == *wanted)
}

/// The lines of a call's text: one line ending at its end is no line of its own, CRLF counts as LF, and an empty text
/// has no line; a text that is only a line ending is one blank line.
fn content_lines(text: &str) -> Vec<&str> {
  if text.is_empty() {
    return Vec::new();
  }
  text
    .strip_suffix("\r\n")
    .or_else(|| text.strip_suffix('\n'))
    .unwrap_or(text)
    .split('\n')
    .map(|line| line.strip_suffix('\r').unwrap_or(line))
    .collect()
}

/// `line` without its ending.
fn bare(line: &str) -> &str {
  let line = line.strip_suffix('\n').unwrap_or(line);
  line.strip_suffix('\r').unwrap_or(line)
}

fn joined(lines: &[&str]) -> String {
  lines.iter().map(|line| format!("{line}\n")).collect()
}

/// `lines` numbered from `first`, within [`MAX_SHOWN_LINES`] and [`MAX_SHOWN_BYTES`].
fn numbered(lines: &[&str], first: u32) -> String {
  if lines.is_empty() {
    return "(no line: the range is empty)\n".to_owned();
  }
  let mut text = String::new();
  for (number, line) in (first..).zip(lines.iter().take(MAX_SHOWN_LINES)) {
    let row = format!("{number:>6}\t{}\n", bare(line));
    if text.len() + row.len() > MAX_SHOWN_BYTES {
      break;
    }
    text.push_str(&row);
  }
  let shown = text.lines().count();
  if shown < lines.len() {
    // Writing to a String cannot fail.
    let _ = writeln!(
      text,
      "[{} more lines of the range left out; read the file again]",
      lines.len() - shown
    );
  }
  text
}

/// Whether `text` has line breaks and every one of them is CRLF.
fn only_crlf(text: &str) -> bool {
  let breaks = text.matches('\n').count();
  breaks > 0 && text.matches("\r\n").count() == breaks
}
