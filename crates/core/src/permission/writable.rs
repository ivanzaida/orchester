//! Extra writable folders: folders outside the task worktree that an agent which edits files may
//! also write to, such as one Cargo build folder shared by every run's worktree
//! (`docs/design/orchester-workspace.md`, "Agent permissions"). They are part of the profile's
//! permissions and apply only with *Edit files in the task worktree*; nothing is granted unless a
//! person adds the folder.
//!
//! A folder is written as an absolute path or as a path relative to the repository root. It must
//! exist, or lie under the repository's `.orchester/` folder, where it is created before the agent
//! starts. Refused, because they would let an agent change what it must not: the repository root,
//! any folder that contains it, anything else inside the user's checkout, `.orchester/` itself and
//! the folders Orchester keeps there (the runs' worktrees, logs), the user's home folder, any folder
//! that contains it, and a file system root. Links are followed before these checks, so a link
//! cannot lead into a refused folder.
//!
//! A stored folder is only checked for its spelling when it is read; the file system can change
//! after it was added, so every use resolves it again ([`WritableFolder::resolve`]).

use std::{
  fmt, fs, io,
  path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::canonical::{self, normalize, plain};

/// The repository's folder for Orchester's own files; writable folders may be created under it.
const STATE_DIR: &str = ".orchester";
/// Folders under [`STATE_DIR`] that Orchester keeps: the runs' worktrees and the runner's logs.
const RESERVED: [&str; 2] = ["worktrees", "logs"];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WritableFolderError {
  #[error("a folder must not be blank")]
  Blank,
  #[error("a folder is at most {max} characters long", max = WritableFolder::MAX_CHARS)]
  TooLong,
  #[error("a folder must not contain control characters")]
  ControlCharacter,
  #[error("the repository root cannot be writable: it is the user's checkout")]
  RepositoryRoot,
  #[error("{0} contains the repository, so the user's checkout would be writable")]
  ContainsRepository(PathBuf),
  #[error("{0} is inside the user's checkout; only folders under .orchester/ can be writable there")]
  InCheckout(PathBuf),
  #[error("{0} holds Orchester's own files")]
  OrchesterFolder(PathBuf),
  #[error("the home folder cannot be writable")]
  Home,
  #[error("{0} contains the home folder")]
  ContainsHome(PathBuf),
  #[error("a file system root cannot be writable")]
  FilesystemRoot,
  #[error("{0} is a file, not a folder")]
  NotAFolder(PathBuf),
  #[error("{0} does not exist; only folders under .orchester/ are created")]
  Missing(PathBuf),
  #[error("{path} cannot be read: {kind}")]
  Unreadable { path: PathBuf, kind: io::ErrorKind },
}

/// Where writable folders are checked against: the repository (relative folders start there) and
/// the user's home folder, both with links resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderBounds {
  repository: PathBuf,
  home: Option<PathBuf>,
}

impl FolderBounds {
  /// Bounds for the repository at `repository` (which must exist) and the home folder `home`, if
  /// it is known.
  pub fn new(repository: &Path, home: Option<&Path>) -> io::Result<Self> {
    let repository = plain(fs::canonicalize(repository)?);
    let home = home.map(|home| fs::canonicalize(home).map_or_else(|_| normalize(home), plain));
    Ok(Self { repository, home })
  }

  pub fn repository(&self) -> &Path {
    &self.repository
  }
}

/// A folder as it will be given to the agent's CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedFolder {
  path: PathBuf,
  missing: bool,
}

impl ResolvedFolder {
  /// The absolute folder, links resolved.
  pub fn path(&self) -> &Path {
    &self.path
  }

  /// The folder does not exist yet; it lies under `.orchester/` and is created before the agent
  /// starts.
  pub const fn is_missing(&self) -> bool {
    self.missing
  }
}

/// One extra writable folder, as the person wrote it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WritableFolder(String);

impl WritableFolder {
  pub const MAX_CHARS: usize = 1024;

  /// `entry` (surrounding spaces removed) if it names a folder an agent may write to within
  /// `bounds`.
  pub fn new(entry: &str, bounds: &FolderBounds) -> Result<Self, WritableFolderError> {
    let folder = Self::spelled(entry)?;
    folder.resolve(bounds)?;
    Ok(folder)
  }

  fn spelled(entry: &str) -> Result<Self, WritableFolderError> {
    let entry = entry.trim();
    if entry.is_empty() {
      return Err(WritableFolderError::Blank);
    }
    if entry.chars().count() > Self::MAX_CHARS {
      return Err(WritableFolderError::TooLong);
    }
    if entry.chars().any(char::is_control) {
      return Err(WritableFolderError::ControlCharacter);
    }
    Ok(Self(entry.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The folder as an absolute path without `.` and `..`, relative to `repository` when it is
  /// relative; the file system is not read.
  pub fn lexical_path(&self, repository: &Path) -> PathBuf {
    normalize(&repository.join(&self.0))
  }

  /// The folder an agent gets, or why it may not write there now.
  pub fn resolve(&self, bounds: &FolderBounds) -> Result<ResolvedFolder, WritableFolderError> {
    let lexical = self.lexical_path(&bounds.repository);
    let (path, missing) = existing(&lexical)?;
    if path.parent().is_none() {
      return Err(WritableFolderError::FilesystemRoot);
    }
    check_repository(&path, &bounds.repository)?;
    if let Some(home) = &bounds.home {
      if *home == path {
        return Err(WritableFolderError::Home);
      }
      if home.starts_with(&path) {
        return Err(WritableFolderError::ContainsHome(path));
      }
    }
    if missing && !path.starts_with(bounds.repository.join(STATE_DIR)) {
      return Err(WritableFolderError::Missing(path));
    }
    if !missing && !path.is_dir() {
      return Err(WritableFolderError::NotAFolder(path));
    }
    Ok(ResolvedFolder { path, missing })
  }
}

impl fmt::Display for WritableFolder {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for WritableFolder {
  type Error = WritableFolderError;

  fn try_from(value: String) -> Result<Self, WritableFolderError> {
    Self::spelled(&value)
  }
}

impl From<WritableFolder> for String {
  fn from(folder: WritableFolder) -> Self {
    folder.0
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WritableFoldersError {
  #[error("{0} is listed more than once")]
  Duplicate(WritableFolder),
  #[error("at most {max} folders can be listed", max = WritableFolders::MAX_ENTRIES)]
  TooMany,
}

/// The extra writable folders, in the order they were added. Empty: the agent writes only in its
/// worktree.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<WritableFolder>", into = "Vec<WritableFolder>")]
pub struct WritableFolders(Vec<WritableFolder>);

impl WritableFolders {
  pub const MAX_ENTRIES: usize = 20;

  pub const fn none() -> Self {
    Self(Vec::new())
  }

  pub fn new(entries: Vec<WritableFolder>) -> Result<Self, WritableFoldersError> {
    if entries.len() > Self::MAX_ENTRIES {
      return Err(WritableFoldersError::TooMany);
    }
    for (index, entry) in entries.iter().enumerate() {
      if entries[..index].contains(entry) {
        return Err(WritableFoldersError::Duplicate(entry.clone()));
      }
    }
    Ok(Self(entries))
  }

  pub fn entries(&self) -> &[WritableFolder] {
    &self.0
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Every folder resolved, in order, without repeats; the first one that cannot be used stops it.
  pub fn resolve(&self, bounds: &FolderBounds) -> Result<Vec<ResolvedFolder>, (WritableFolder, WritableFolderError)> {
    let mut resolved: Vec<ResolvedFolder> = Vec::new();
    for folder in &self.0 {
      let folder_path = folder.resolve(bounds).map_err(|error| (folder.clone(), error))?;
      if !resolved.iter().any(|known| known.path == folder_path.path) {
        resolved.push(folder_path);
      }
    }
    Ok(resolved)
  }
}

impl TryFrom<Vec<WritableFolder>> for WritableFolders {
  type Error = WritableFoldersError;

  fn try_from(entries: Vec<WritableFolder>) -> Result<Self, WritableFoldersError> {
    Self::new(entries)
  }
}

impl From<WritableFolders> for Vec<WritableFolder> {
  fn from(folders: WritableFolders) -> Self {
    folders.0
  }
}

/// Refuses `path` when it is the repository, contains it, or lies in the checkout or in
/// Orchester's own folders.
fn check_repository(path: &Path, repository: &Path) -> Result<(), WritableFolderError> {
  if path == repository {
    return Err(WritableFolderError::RepositoryRoot);
  }
  if repository.starts_with(path) {
    return Err(WritableFolderError::ContainsRepository(path.to_path_buf()));
  }
  let Ok(inside) = path.strip_prefix(repository) else {
    return Ok(());
  };
  let mut parts = inside.components().map(Component::as_os_str);
  if parts.next().and_then(|part| part.to_str()) != Some(STATE_DIR) {
    return Err(WritableFolderError::InCheckout(path.to_path_buf()));
  }
  match parts.next().and_then(|part| part.to_str()) {
    Some(name) if !RESERVED.iter().any(|reserved| reserved.eq_ignore_ascii_case(name)) => Ok(()),
    _ => Err(WritableFolderError::OrchesterFolder(path.to_path_buf())),
  }
}

/// `path` with links resolved as far as it exists, and whether part of it does not exist yet.
fn existing(path: &Path) -> Result<(PathBuf, bool), WritableFolderError> {
  match canonical::existing(path) {
    Ok(found) => Ok((found.path, found.missing)),
    Err(failure) if failure.error.kind() == io::ErrorKind::NotFound => Err(WritableFolderError::Missing(failure.path)),
    Err(failure) => Err(WritableFolderError::Unreadable {
      path: failure.path,
      kind: failure.error.kind(),
    }),
  }
}
