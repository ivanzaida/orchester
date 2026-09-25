//! Keeps `.orchester/` out of git through the repository's own `info/exclude`, so the
//! project's committed `.gitignore` is never modified.

use std::{
  fs,
  io::ErrorKind,
  path::{Path, PathBuf},
};

use crate::StoreError;

/// Adds `pattern` to the repository's exclude file unless an identical line exists.
pub(crate) fn exclude(repo_root: &Path, pattern: &str) -> Result<(), StoreError> {
  let info = common_git_dir(repo_root)?.join("info");
  fs::create_dir_all(&info).map_err(StoreError::io(&info))?;
  let file = info.join("exclude");
  let existing = match fs::read_to_string(&file) {
    Ok(contents) => contents,
    Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
    Err(error) => return Err(StoreError::io(&file)(error)),
  };
  if existing.lines().any(|line| line.trim() == pattern) {
    return Ok(());
  }
  let separator = if existing.is_empty() || existing.ends_with('\n') {
    ""
  } else {
    "\n"
  };
  fs::write(&file, format!("{existing}{separator}{pattern}\n")).map_err(StoreError::io(&file))
}

/// Whether `repo_root` is the top of a git repository or of a linked worktree: what creating a
/// project there requires.
pub fn is_repository_root(repo_root: &Path) -> bool {
  let dot_git = repo_root.join(".git");
  dot_git.is_dir() || dot_git.is_file()
}

/// The directory holding `info/exclude`: `.git` itself, or for a linked worktree (`.git` is a
/// file) the common directory of the main repository.
fn common_git_dir(repo_root: &Path) -> Result<PathBuf, StoreError> {
  let dot_git = repo_root.join(".git");
  if dot_git.is_dir() {
    return Ok(dot_git);
  }
  if !dot_git.is_file() {
    return Err(StoreError::NotAGitRepository(repo_root.to_path_buf()));
  }
  let pointer = fs::read_to_string(&dot_git).map_err(StoreError::io(&dot_git))?;
  let git_dir = pointer
    .strip_prefix("gitdir:")
    .map(|path| repo_root.join(path.trim()))
    .ok_or_else(|| StoreError::NotAGitRepository(repo_root.to_path_buf()))?;
  let common_pointer = git_dir.join("commondir");
  match fs::read_to_string(&common_pointer) {
    Ok(common) => Ok(git_dir.join(common.trim())),
    Err(error) if error.kind() == ErrorKind::NotFound => Ok(git_dir),
    Err(error) => Err(StoreError::io(&common_pointer)(error)),
  }
}
