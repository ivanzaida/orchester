//! `read_outside_file`: part of a text file outside the worktree, such as a dependency's source. The path
//! is absolute and local and is resolved with its links followed before anything is read. Refused are the
//! credential locations the command classifier knows ([`CommandScope::holds_credentials`]: the home
//! folder's dot files and folders, the applications' data), anything in a `.git` folder (a repository's
//! configuration can hold a remote's credentials), and the folders the runner names (its private files
//! with the invocation's secrets, the project's own `.orchester` folder), but for the worktree within them.
//! The answer is numbered lines, cut to [`MAX_READ_BYTES`].

use std::{
  fmt::Write,
  fs::File,
  io::{self, BufRead, BufReader},
  path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{
  native::scope::{ScopeError, in_git_folder, resolve_local},
  permission::{canonical::existing, category::CommandScope},
};

/// The most a read answers with, in bytes of the file's lines.
pub const MAX_READ_BYTES: usize = 256 * 1024;

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
    let refused =
      !resolved.starts_with(self.scope.worktree()) && self.refused.iter().any(|folder| resolved.starts_with(folder));
    if in_git_folder(&resolved) || refused {
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

  /// Lines `offset` (from 1) to `offset + limit - 1` of `path`, numbered, at most [`MAX_READ_BYTES`] of
  /// them, with a note when the file goes on.
  pub fn read(&self, path: &str, offset: u32, limit: u32) -> Result<String, OutsideReadError> {
    let file = self.check(path)?;
    let unreadable = |error: io::Error| OutsideReadError::Unreadable {
      path: file.clone(),
      kind: error.kind(),
    };
    let mut reader = BufReader::new(File::open(&file).map_err(unreadable)?);
    let (mut text, mut line, mut number, mut bytes) = (String::new(), Vec::new(), 0_u32, 0_usize);
    let last = offset.saturating_add(limit.saturating_sub(1));
    loop {
      line.clear();
      if reader.read_until(b'\n', &mut line).map_err(unreadable)? == 0 {
        return Ok(text);
      }
      number += 1;
      if line.contains(&0) {
        return Err(OutsideReadError::Binary(file));
      }
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
      let _ = writeln!(text, "{number:>6}\t{}", content.trim_end_matches(['\n', '\r']));
    }
  }
}
