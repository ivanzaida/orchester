//! `read_file` (`docs/architecture/native-agent.md` §4.1): numbered lines from a 1-based offset, each
//! cut at 2000 characters; a binary file (a NUL byte in its first 8 KiB) is named with its size only. Orchester's own
//! `read_file` for agent CLIs reads the same way, naming its fields `start_line` and `length`
//! ([`FileTools::read_lines`]). A PNG, JPEG or WebP file ([`ImageKind`]) is sent as an image to Claude Code
//! ([`FileTools::read_shown`]); every other agent reads a note that names it and says it is not shown.

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
  FileError, FileTools, count, done, failed,
  image::{ImageKind, ShownImage},
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
/// What a native agent reads after an image's name and size.
const NOT_SHOWN_NATIVE: &str = "Images are not shown to native agents yet.";
/// What an agent CLI that is not shown images (Codex) reads after an image's name and size.
const NOT_SHOWN_CLI: &str = "Images are shown on Claude Code connections only.";

/// What Orchester's `read_file` answers an agent that is shown images ([`FileTools::read_shown`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadAnswer {
  /// Numbered lines, a binary file's size, or why the read failed.
  Text(ToolOutcome),
  /// The file is a PNG, JPEG or WebP image.
  Image(ShownImage),
}

impl FileTools {
  pub fn read_file(&self, input: &ReadFile) -> ToolOutcome {
    let range = (input.offset(), input.limit());
    outcome(self.read(input.path(), range, "offset", NOT_SHOWN_NATIVE))
  }

  /// Orchester's `read_file`: as the native agent's, its note naming `start_line` to read on.
  pub fn read_lines(&self, input: &ReadLines) -> ToolOutcome {
    let range = (input.start_line(), input.length());
    outcome(self.read(input.filename(), range, "start_line", NOT_SHOWN_CLI))
  }

  /// Orchester's `read_file` for an agent that is shown images: an image file as the image, whatever `start_line` and
  /// `length` say, else as [`Self::read_lines`] reads it.
  pub fn read_shown(&self, input: &ReadLines) -> ReadAnswer {
    let read = self
      .load(input.filename())
      .and_then(|(path, bytes)| match ImageKind::of(&bytes) {
        Some(kind) => ShownImage::of(&bytes, kind)
          .map(ReadAnswer::Image)
          .map_err(|error| FileError::Image { path, error }),
        None => Ok(ReadAnswer::Text(lines_of(
          &bytes,
          (input.start_line(), input.length()),
          "start_line",
        ))),
      });
    read.unwrap_or_else(|error| ReadAnswer::Text(failed(&error)))
  }

  /// Lines `offset..offset + limit` of the file at `requested`; a cut answer names the field `on` to read on with. An
  /// image is named with its size and `not_shown`.
  fn read(&self, requested: &str, range: (u32, u32), on: &str, not_shown: &str) -> Result<ToolOutcome, FileError> {
    let (_, bytes) = self.load(requested)?;
    let Some(kind) = ImageKind::of(&bytes) else {
      return Ok(lines_of(&bytes, range, on));
    };
    let text = kind.note(&bytes, not_shown);
    let none = CallDetail::Read {
      first: 0,
      last: 0,
      bytes: count(bytes.len()),
    };
    Ok(done(text.clone(), text, Vec::new(), none))
  }

  /// The content of the file at `requested`, its read stamped, and its path as the agent sees it.
  fn load(&self, requested: &str) -> Result<(String, Vec<u8>), FileError> {
    let path = self.scope.readable(requested)?;
    let shown = self.shown(&path);
    let metadata = fs::metadata(&path).map_err(|error| self.io_error(&path, "read", &error))?;
    if metadata.is_dir() {
      return Err(FileError::Folder(shown));
    }
    let bytes = self.content(&path, &metadata)?;
    self.stamps.record(&path, ReadStamp::of(&bytes, &metadata));
    Ok((shown, bytes))
  }
}

/// Lines `offset..offset + limit` of a file holding `bytes`, or its size when it is binary.
fn lines_of(bytes: &[u8], (offset, limit): (u32, u32), on: &str) -> ToolOutcome {
  let size = count(bytes.len());
  if bytes.iter().take(BINARY_PROBE).any(|byte| *byte == 0) {
    let text = format!("binary file, {} bytes", bytes.len());
    let none = CallDetail::Read {
      first: 0,
      last: 0,
      bytes: size,
    };
    return done(text.clone(), text, Vec::new(), none);
  }
  let text = String::from_utf8_lossy(bytes);
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
  done(content, activity, Vec::new(), detail)
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
