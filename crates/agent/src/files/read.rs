//! `read_file` (`docs/architecture/native-agent.md` §4.1): numbered lines from a 1-based offset, each
//! cut at 2000 characters; a binary file (a NUL byte in its first 8 KiB) is named with its size only.

use std::fs;

use orchester_core::native::{session::ToolOutcome, tools::ReadFile};

use super::{
  FileError, FileTools, done,
  limits::{RESULT_CAP, cut_line},
  outcome,
  stamp::ReadStamp,
};

/// Characters a line keeps.
const MAX_LINE_CHARS: usize = 2000;
/// Bytes looked at for a NUL to tell a binary file.
const BINARY_PROBE: usize = 8 * 1024;
/// The largest file `read_file` reads: it reads the whole file to stamp it.
pub(super) const MAX_READ_BYTES: u64 = 32 * 1024 * 1024;
/// Room kept below [`RESULT_CAP`] for the note that says where to read on.
const NOTE_ROOM: usize = 256;

impl FileTools {
  pub fn read_file(&self, input: &ReadFile) -> ToolOutcome {
    outcome(self.read(input))
  }

  fn read(&self, input: &ReadFile) -> Result<ToolOutcome, FileError> {
    let path = self.scope.readable(input.path())?;
    let shown = self.shown(&path);
    let metadata = fs::metadata(&path).map_err(|error| self.io_error(&path, "read", &error))?;
    if metadata.is_dir() {
      return Err(FileError::Folder(shown));
    }
    if metadata.len() > MAX_READ_BYTES {
      return Err(FileError::TooLarge {
        path: shown,
        size: metadata.len(),
        limit: MAX_READ_BYTES,
      });
    }
    let bytes = fs::read(&path).map_err(|error| self.io_error(&path, "read", &error))?;
    self.stamps.record(&path, ReadStamp::of(&bytes, &metadata));
    if bytes.iter().take(BINARY_PROBE).any(|byte| *byte == 0) {
      let text = format!("binary file, {} bytes", bytes.len());
      return Ok(done(text.clone(), text, Vec::new()));
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Ok(numbered(text, input.offset(), input.limit()))
  }
}

/// Lines `offset..offset + limit` of `text`, numbered, with a note when more follow.
fn numbered(text: &str, offset: u32, limit: u32) -> ToolOutcome {
  let lines: Vec<&str> = text.lines().collect();
  let total = lines.len();
  if total == 0 {
    return done("(empty file)".to_owned(), "empty file".to_owned(), Vec::new());
  }
  let first = usize::try_from(offset).unwrap_or(usize::MAX);
  if first > total {
    let text = format!("the file has {total} lines; offset {offset} is past its end");
    return done(text.clone(), text, Vec::new());
  }
  let wanted = usize::try_from(limit).unwrap_or(usize::MAX);
  let mut out = String::new();
  let mut last = first - 1;
  for (index, line) in lines.iter().enumerate().skip(first - 1).take(wanted) {
    let row = format!("{:>6}\t{}\n", index + 1, cut_line(line, MAX_LINE_CHARS));
    if out.len() + row.len() > RESULT_CAP - NOTE_ROOM {
      break;
    }
    out.push_str(&row);
    last = index + 1;
  }
  if last < total {
    out.push_str(&format!(
      "[truncated: showing lines {first}–{last} of {total}; call read_file with offset {} to read on]",
      last + 1
    ));
  }
  done(out, format!("lines {first}–{last} of {total}"), Vec::new())
}
