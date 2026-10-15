//! A file's path as the code viewer takes it: relative to the project's folder (or a run's worktree),
//! `/`-separated, never stepping out with `..`, never absolute, and never inside `.git` or `.orchester`
//! (`docs/design/code-viewer.md`, "The page"). Whether a link on the way leads out is decided where the
//! file is read, by the file scope agents' file tools use.

use std::{fmt, sync::Arc};

use thiserror::Error;

const GIT_DIR: &str = ".git";
const STATE_DIR: &str = ".orchester";

/// Why a path is not one the viewer opens.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PathError {
  #[error("no path")]
  Empty,
  #[error("`{0}` is absolute")]
  Absolute(String),
  #[error("`{0}` steps out of its folder with `..`")]
  Parent(String),
  #[error("`{0}` contains a backslash or a NUL character")]
  Invalid(String),
  #[error("`{0}` lies in `.git` or `.orchester`")]
  Hidden(String),
  #[error("`{0}` leads out of the folder through a link or junction")]
  Outside(String),
}

/// A file's path inside the project's folder, `/`-separated, without `.` and empty parts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilePath(Arc<str>);

impl FilePath {
  /// `value` as a project path, or why it is not one. Backslashes are refused rather than read as
  /// separators, so a path means the same on every platform.
  pub fn new(value: &str) -> Result<Self, PathError> {
    if value.contains(['\\', '\0']) {
      return Err(PathError::Invalid(value.to_owned()));
    }
    if value.starts_with('/') || has_drive(value) {
      return Err(PathError::Absolute(value.to_owned()));
    }
    let parts: Vec<&str> = value
      .split('/')
      .filter(|part| !part.is_empty() && *part != ".")
      .collect();
    if parts.contains(&"..") {
      return Err(PathError::Parent(value.to_owned()));
    }
    if parts.is_empty() {
      return Err(PathError::Empty);
    }
    let path = parts.join("/");
    if is_hidden(&path) {
      return Err(PathError::Hidden(value.to_owned()));
    }
    Ok(Self(path.into()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  pub fn shared(&self) -> Arc<str> {
    self.0.clone()
  }

  /// The last part of the path.
  pub fn name(&self) -> &str {
    self.0.rsplit('/').next().unwrap_or(&self.0)
  }

  /// The folders the file lies in, outermost first: `a`, `a/b` for `a/b/c.rs`.
  pub fn folders(&self) -> impl Iterator<Item = &str> {
    self.0.match_indices('/').map(|(at, _)| &self.0[..at])
  }
}

impl fmt::Display for FilePath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

/// Whether the `/`-separated project path lies in `.git` (at any depth) or in `.orchester` (at the top),
/// names compared as Windows and macOS open them, without regard to case.
pub fn is_hidden(path: &str) -> bool {
  path
    .split('/')
    .enumerate()
    .any(|(depth, part)| part.eq_ignore_ascii_case(GIT_DIR) || (depth == 0 && part.eq_ignore_ascii_case(STATE_DIR)))
}

/// Whether `value` starts with a drive letter (`C:`), which is absolute or drive-relative on Windows and
/// is refused on every platform so a link means the same everywhere.
fn has_drive(value: &str) -> bool {
  let mut chars = value.chars();
  matches!((chars.next(), chars.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic())
}
