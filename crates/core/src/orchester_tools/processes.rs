//! Process tools are confined to the executable, never its working directory or command line.

use std::{
  io,
  num::NonZeroU32,
  path::{Component, Path, PathBuf, Prefix},
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
  /// The worktree with links resolved and long names.
  root: PathBuf,
  /// The worktree as the run was given it, absolute, so a program started through that spelling is recognised.
  given: PathBuf,
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
    let given = std::path::absolute(worktree).map_err(|error| ProcessScopeError::Unresolved {
      path: worktree.to_path_buf(),
      kind: error.kind(),
    })?;
    Ok(Self {
      root,
      given: canonical::normalize(&given),
    })
  }

  /// The executable's resolved path relative to this run, or none when it lies outside.
  ///
  /// The path as the OS reports it must already lie inside the worktree, and so must the path with links resolved:
  /// a link from outside into the worktree never makes an outside program stoppable. Missing paths and links that
  /// cannot be resolved fail closed.
  pub fn relative_executable(&self, executable: &Path) -> Result<Option<PathBuf>, ProcessScopeError> {
    if !spelled_inside(executable, &self.root) && !spelled_inside(executable, &self.given) {
      return Ok(None);
    }
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

/// Whether `path`, as written, lies inside `base` by whole components. Case differs only in ASCII letters, which
/// never makes two distinct names alias; a `..` component fails closed.
fn spelled_inside(path: &Path, base: &Path) -> bool {
  if !path.is_absolute() || path.components().any(|component| component == Component::ParentDir) {
    return false;
  }
  let mut components = path.components();
  base
    .components()
    .all(|expected| components.next().is_some_and(|found| same_component(found, expected)))
}

fn same_component(found: Component<'_>, expected: Component<'_>) -> bool {
  match (found, expected) {
    (Component::Prefix(found), Component::Prefix(expected)) => same_prefix(found.kind(), expected.kind()),
    (Component::Normal(found), Component::Normal(expected)) => found.eq_ignore_ascii_case(expected),
    (found, expected) => found == expected,
  }
}

/// `C:` and `\\?\C:` (and the two UNC spellings) name the same place.
fn same_prefix(found: Prefix<'_>, expected: Prefix<'_>) -> bool {
  match (found, expected) {
    (Prefix::Disk(found) | Prefix::VerbatimDisk(found), Prefix::Disk(expected) | Prefix::VerbatimDisk(expected)) => {
      found.eq_ignore_ascii_case(&expected)
    }
    (
      Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share),
      Prefix::UNC(expected_server, expected_share) | Prefix::VerbatimUNC(expected_server, expected_share),
    ) => server.eq_ignore_ascii_case(expected_server) && share.eq_ignore_ascii_case(expected_share),
    (found, expected) => found == expected,
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
