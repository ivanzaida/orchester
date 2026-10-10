//! Process tools are confined to the resolved executable, never its working directory or command line.

use std::{
  io,
  num::NonZeroU32,
  path::{Path, PathBuf},
};

use thiserror::Error;

use crate::permission::canonical;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessCall {
  List,
  Stop { pid: NonZeroU32 },
}

/// A run's executable boundary, resolved once when the invocation starts.
#[derive(Clone, Debug)]
pub struct RunProcessScope {
  root: PathBuf,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProcessScopeError {
  #[error("cannot resolve process scope path {}: {kind}", .path.display())]
  Unresolved { path: PathBuf, kind: io::ErrorKind },
  #[error("the run's worktree is not a directory: {}", .0.display())]
  NotDirectory(PathBuf),
}

impl RunProcessScope {
  pub fn new(worktree: &Path) -> Result<Self, ProcessScopeError> {
    let root = resolve(worktree)?;
    if !root.is_dir() {
      return Err(ProcessScopeError::NotDirectory(root));
    }
    Ok(Self { root })
  }

  /// The executable's resolved path relative to this run, or none when it lies outside.
  /// Missing paths and links that cannot be resolved fail closed.
  pub fn relative_executable(&self, executable: &Path) -> Result<Option<PathBuf>, ProcessScopeError> {
    let resolved = resolve(executable)?;
    // Compare OS-final components, not folded user strings: Windows names such as ß and ss need not be aliases.
    if !resolved.starts_with(&self.root) || !resolved.is_file() {
      return Ok(None);
    }
    Ok(Some(
      resolved.components().skip(self.root.components().count()).collect(),
    ))
  }
}

fn resolve(path: &Path) -> Result<PathBuf, ProcessScopeError> {
  canonical::canonicalize(path)
    .map(canonical::plain)
    .map_err(|error| ProcessScopeError::Unresolved {
      path: path.to_path_buf(),
      kind: error.kind(),
    })
}
