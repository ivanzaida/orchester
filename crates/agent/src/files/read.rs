//! `read_file` (`docs/architecture/native-agent.md` §4.1): numbered lines from a 1-based offset, each
//! cut at 2000 characters; a binary file (a NUL byte in its first 8 KiB) is named with its size only. Orchester's own
//! `read_file` for agent CLIs reads the same way, naming its fields `start_line` and `length`
//! ([`FileTools::read_lines`]).

use std::{
  fs,
  io::{self, Read as _},
  path::Path,
};

use orchester_core::{
  connection::stream::CallDetail,
  native::{session::ToolOutcome, tools::ReadFile},
  orchester_tools::ReadLines,
};

use super::{
  FileError, FileTools, count, done,
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
    outcome(self.read(input.path(), (input.offset(), input.limit()), "offset"))
  }

  /// Orchester's `read_file`: as the native agent's, its note naming `start_line` to read on.
  pub fn read_lines(&self, input: &ReadLines) -> ToolOutcome {
    outcome(self.read(input.filename(), (input.start_line(), input.length()), "start_line"))
  }

  /// Lines `offset..offset + limit` of the file at `requested`, its read stamped; a cut answer names the field
  /// `on` to read on with.
  fn read(&self, requested: &str, (offset, limit): (u32, u32), on: &str) -> Result<ToolOutcome, FileError> {
    let path = self.scope.readable(requested)?;
    let shown = self.shown(&path);
    let metadata = fs::metadata(&path).map_err(|error| self.io_error(&path, "read", &error))?;
    if metadata.is_dir() {
      return Err(FileError::Folder(shown));
    }
    let bytes = self.content(&path, &metadata)?;
    self.stamps.record(&path, ReadStamp::of(&bytes, &metadata));
    let size = count(bytes.len());
    if bytes.iter().take(BINARY_PROBE).any(|byte| *byte == 0) {
      let text = format!("binary file, {} bytes", bytes.len());
      let none = CallDetail::Read {
        first: 0,
        last: 0,
        bytes: size,
      };
      return Ok(done(text.clone(), text, Vec::new(), none));
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let Numbered {
      content,
      activity,
      first,
      last,
    } = numbered(text, (offset, limit), on);
    let detail = CallDetail::Read {
      first,
      last,
      bytes: size,
    };
    Ok(done(content, activity, Vec::new(), detail))
  }
}

/// The lines a read shows, and which they are: none when both are 0.
struct Numbered {
  content: String,
  activity: String,
  first: u64,
  last: u64,
}

impl FileTools {
  /// The whole content of the regular file at `path`, at most [`MAX_READ_BYTES`]. A FIFO, a device or
  /// a socket is refused before it is opened, since reading one can wait forever; a file that grows
  /// past the limit while it is read is refused as well.
  pub(super) fn content(&self, path: &Path, metadata: &fs::Metadata) -> Result<Vec<u8>, FileError> {
    let too_large = |size: u64| FileError::TooLarge {
      path: self.shown(path),
      size,
      limit: MAX_READ_BYTES,
    };
    if !metadata.is_file() {
      return Err(FileError::NotRegular(self.shown(path)));
    }
    if metadata.len() > MAX_READ_BYTES {
      return Err(too_large(metadata.len()));
    }
    let fail = |error: io::Error| self.io_error(path, "read", &error);
    let mut bytes = Vec::new();
    fs::File::open(path)
      .map_err(fail)?
      .take(MAX_READ_BYTES + 1)
      .read_to_end(&mut bytes)
      .map_err(fail)?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_READ_BYTES {
      return Err(too_large(read));
    }
    Ok(bytes)
  }
}

/// Lines `offset..offset + limit` of `text`, numbered, with a note naming the field `on` when more follow.
fn numbered(text: &str, (offset, limit): (u32, u32), on: &str) -> Numbered {
  let none = |content: String, activity: String| Numbered {
    content,
    activity,
    first: 0,
    last: 0,
  };
  let lines: Vec<&str> = text.lines().collect();
  let total = lines.len();
  if total == 0 {
    return none("(empty file)".to_owned(), "empty file".to_owned());
  }
  let first = usize::try_from(offset).unwrap_or(usize::MAX);
  if first > total {
    let text = format!("the file has {total} lines; {on} {offset} is past its end");
    return none(text.clone(), text);
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
      "[truncated: showing lines {first}–{last} of {total}; call read_file with {on} {} to read on]",
      last + 1
    ));
  }
  Numbered {
    content: out,
    activity: format!("lines {first}–{last} of {total}"),
    first: count(first),
    last: count(last),
  }
}
