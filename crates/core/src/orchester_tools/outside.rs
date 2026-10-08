//! `read_outside_file`: part of a text file outside the worktree, such as a dependency's source. The path
//! is absolute and local and is resolved with its links followed before anything is read.
//!
//! What it may reach is the whole disk but a deny list, not an allow list of dependency folders: the
//! folders where toolchains keep sources (`~/.cargo/registry/src`, `~/go/pkg/mod`, `~/.m2`, a toolchain's
//! library sources, …) differ by toolchain and machine, and Orchester names no toolchain's folders (the
//! user's decision of 2026-10-02, "Cargo-specific build folder handling removed"). Every read asks a person
//! unless a rule of the run or auto-approval answers it, and refused are:
//!
//! - the credential locations the command classifier knows ([`CommandScope::holds_credentials`]: the home folder's dot
//!   files and folders, the applications' data in `AppData` and `Library`);
//! - anything in a `.git` folder (a repository's configuration can hold a remote's credentials);
//! - Orchester's own state wherever it lies: any `.orchester` folder (every project's database, memory and runs'
//!   worktrees), an application or shared server database (`orchester.db`, `orchester-server.db`, their journals), and
//!   the folders the runner names (every project's private files with MCP secrets and tokens, the folder of Orchester's
//!   programs and database); the run's own worktree stays readable.
//!
//! Known limits: a path is checked, then opened, so a link swapped in between by another program could lead
//! the read elsewhere; a mapped network drive (`Z:\`) or a mounted share (`/Volumes/…`) reads as a local
//! folder, also in a step without network. The answer is numbered lines, each cut to [`MAX_LINE_BYTES`], at
//! most [`MAX_READ_BYTES`] of them; the file is read as it comes, never held whole.

use std::{
  fmt::Write,
  fs::File,
  io::{self, BufRead, BufReader},
  path::{Component, Path, PathBuf},
};

use thiserror::Error;

use crate::{
  native::scope::{ScopeError, in_git_folder, resolve_local},
  permission::{canonical::existing, category::CommandScope},
};

/// The most a read answers with, in bytes of the file's lines.
pub const MAX_READ_BYTES: usize = 56 * 1024;
/// The most of one line a read answers with.
pub const MAX_LINE_BYTES: usize = 16 * 1024;
/// The folder of Orchester's state in a repository.
const STATE_DIR: &str = ".orchester";
/// The application's and the shared server's databases, and the files SQLite keeps beside them.
const DATABASES: [&str; 2] = ["orchester.db", "orchester-server.db"];

/// Why a file outside the worktree is not read. The message is the error result the agent gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OutsideReadError {
  #[error("`{0}` is not an absolute path")]
  NotAbsolute(String),
  #[error(transparent)]
  Path(#[from] ScopeError),
  #[error("{} is a credential location, which Orchester's tools never read", .0.display())]
  Credentials(PathBuf),
  #[error("{} is protected: Orchester's tools never read there", .0.display())]
  Protected(PathBuf),
  #[error("{} is not a file", .0.display())]
  NotAFile(PathBuf),
  #[error("{} is not a text file", .0.display())]
  Binary(PathBuf),
  #[error("{} cannot be read: {kind}", .path.display())]
  Unreadable { path: PathBuf, kind: io::ErrorKind },
}

/// What reads outside the worktree may reach in one step.
#[derive(Clone, Debug)]
pub struct OutsideReads {
  scope: CommandScope,
  refused: Vec<PathBuf>,
}

impl OutsideReads {
  /// Reads by a step whose commands run in `scope`, never in `refused` or below, but in the worktree. Each
  /// refused folder is resolved now as far as it exists, so one created later (the private files) is still
  /// told by the canonical form of its existing parent.
  pub fn new(scope: CommandScope, refused: &[PathBuf]) -> Self {
    let refused = refused
      .iter()
      // A folder whose parents cannot be read is refused as it is written.
      .map(|folder| existing(folder).map_or_else(|_| folder.clone(), |found| found.path))
      .collect();
    Self { scope, refused }
  }

  /// The file `path` names, resolved, or why it may not be read.
  pub fn check(&self, path: &str) -> Result<PathBuf, OutsideReadError> {
    if !Path::new(path).is_absolute() {
      return Err(OutsideReadError::NotAbsolute(path.to_owned()));
    }
    let resolved = resolve_local(path)?;
    if in_git_folder(&resolved) || self.orchester_state(&resolved) {
      return Err(OutsideReadError::Protected(resolved));
    }
    if self.scope.holds_credentials(&resolved) {
      return Err(OutsideReadError::Credentials(resolved));
    }
    if !resolved.is_file() {
      return Err(OutsideReadError::NotAFile(resolved));
    }
    Ok(resolved)
  }

  /// Whether `path` (resolved) is Orchester's own state; the worktree is the run's own.
  fn orchester_state(&self, path: &Path) -> bool {
    if path.starts_with(self.scope.worktree()) {
      return false;
    }
    let named = path.components().any(|component| match component {
      Component::Normal(name) => {
        let name = name.to_string_lossy().to_lowercase();
        name == STATE_DIR || DATABASES.iter().any(|database| name.starts_with(database))
      }
      _ => false,
    });
    named || self.refused.iter().any(|folder| path.starts_with(folder))
  }

  /// Lines `offset` (from 1) to `offset + limit - 1` of `path`, numbered, as [`read_lines`] reads them.
  pub fn read(&self, path: &str, offset: u32, limit: u32) -> Result<String, OutsideReadError> {
    read_lines(self.check(path)?, offset, limit)
  }
}

/// Lines `offset` (from 1) to `offset + limit - 1` of `file`, a resolved path, numbered, at most
/// [`MAX_READ_BYTES`] of them, with a note when the file goes on. Lines before `offset` are skipped without
/// being kept.
pub(super) fn read_lines(file: PathBuf, offset: u32, limit: u32) -> Result<String, OutsideReadError> {
  let unreadable = |error: io::Error| OutsideReadError::Unreadable {
    path: file.clone(),
    kind: error.kind(),
  };
  let opened = File::open(&file).map_err(unreadable)?;
  // What was opened must still be a file: a folder or a pipe swapped in since the check is refused.
  if !opened.metadata().map_err(unreadable)?.is_file() {
    return Err(OutsideReadError::NotAFile(file));
  }
  let mut reader = BufReader::new(opened);
  let (mut text, mut line, mut number, mut bytes) = (String::new(), Vec::new(), 0_u32, 0_usize);
  let last = offset.saturating_add(limit.saturating_sub(1));
  loop {
    let keep = if number + 1 < offset { 0 } else { MAX_LINE_BYTES };
    let Some(Line { cut, nul }) = next_line(&mut reader, keep, &mut line).map_err(unreadable)? else {
      return Ok(text);
    };
    // A NUL anywhere up to the last line read, skipped lines too, makes it no text file.
    if nul {
      return Err(OutsideReadError::Binary(file));
    }
    number += 1;
    if number < offset {
      continue;
    }
    // Writing to a String cannot fail.
    if number > last || bytes + line.len() > MAX_READ_BYTES {
      let _ = writeln!(text, "[the file goes on: read on from offset {number}]");
      return Ok(text);
    }
    bytes += line.len();
    let content = String::from_utf8_lossy(&line);
    let _ = write!(text, "{number:>6}\t{}", content.trim_end_matches(['\n', '\r']));
    let _ = writeln!(text, "{}", if cut { " [rest of the line left out]" } else { "" });
  }
}

/// What [`next_line`] saw of a line.
struct Line {
  /// Part of it was left out.
  cut: bool,
  /// It holds a NUL, kept or not.
  nul: bool,
}

/// Reads the next line of `reader` into `line`, keeping at most `keep` bytes of it and passing over the
/// rest; `None` at the end of the file.
fn next_line(reader: &mut impl BufRead, keep: usize, line: &mut Vec<u8>) -> io::Result<Option<Line>> {
  line.clear();
  let (mut seen, mut any) = (Line { cut: false, nul: false }, false);
  loop {
    let buffer = reader.fill_buf()?;
    if buffer.is_empty() {
      return Ok(any.then_some(seen));
    }
    any = true;
    let end = buffer.iter().position(|byte| *byte == b'\n');
    let chunk = &buffer[..end.map_or(buffer.len(), |at| at + 1)];
    let room = keep.saturating_sub(line.len());
    seen.cut |= chunk.len() > room;
    seen.nul |= chunk.contains(&0);
    line.extend_from_slice(&chunk[..chunk.len().min(room)]);
    let used = chunk.len();
    reader.consume(used);
    if end.is_some() {
      return Ok(Some(seen));
    }
  }
}
