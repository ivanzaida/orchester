//! Where the native agent's file tools may read and write (`docs/architecture/native-agent.md` §5.1).
//! The agent has no CLI sandbox, so this is the confinement: every path a tool is given is resolved here
//! before it is opened.
//!
//! Readable are the worktree, the code index and skills folders, and the granted writable folders.
//! Writable are the worktree when the step edits files, and the granted writable folders; never a `.git`
//! file or folder at any depth (writing there could plant hooks Orchester's checkpoint commit would run),
//! `.orchester/` inside the worktree, the code index or the skills folder.
//!
//! A path is resolved the way the executor will open it: joined to the worktree, `.` and `..` removed
//! lexically, then its longest existing ancestor canonicalised by the OS, so a symbolic link or junction
//! that leads out of the scope is refused. The roots are canonicalised the same way, which makes
//! `/var` and `/private/var` on macOS compare equal. Reading the file system to canonicalise is the only
//! I/O here. A hard link is a directory entry like any other: it lies where its entry lies, and writes
//! replace the entry by renaming a new file over it (§4.4) instead of writing through it.

#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

use std::{
  fs, io,
  path::{Component, Path, PathBuf},
};

use thiserror::Error;

#[cfg(not(windows))]
use self::unix as platform;
#[cfg(windows)]
use self::windows as platform;
use crate::permission::{
  RepositoryAccess, ResolvedFolder,
  canonical::{self, normalize},
};

/// The repository's folder for Orchester's own files, protected inside the worktree.
const STATE_DIR: &str = ".orchester";
const GIT_DIR: &str = ".git";

/// Why a file tool may not use a path. The message is the error result the model gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ScopeError {
  #[error("the path contains a NUL character")]
  Nul,
  #[error("device and verbatim paths are not allowed: {0}")]
  DevicePath(String),
  #[error("`{0}` names a Windows device")]
  DeviceName(String),
  #[error("alternate data streams are not allowed: {0}")]
  AlternateStream(String),
  #[error("drive-relative paths are not allowed: {0}")]
  DriveRelative(String),
  #[error("network paths are allowed only inside a writable folder: {0}")]
  NetworkPath(String),
  #[error("{} is outside the folders this step may read", .0.display())]
  NotReadable(PathBuf),
  #[error("{} is outside the folders this step may write", .0.display())]
  NotWritable(PathBuf),
  #[error("{} is protected: the agent never writes there", .0.display())]
  Protected(PathBuf),
  #[error("{} cannot be resolved: {kind}", .path.display())]
  Unresolved { path: PathBuf, kind: io::ErrorKind },
}

/// How a path is written, after the checks of [`platform::path_form`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathForm {
  Local,
  /// A Windows network path (`\\server\share\…`).
  Network,
}

/// The folders that make up a step's scope, as the runner knows them.
#[derive(Clone, Copy, Debug)]
pub struct ScopeRoots<'a> {
  pub worktree: &'a Path,
  pub access: RepositoryAccess,
  /// The code index of the run's revision, read-only.
  pub code_index: Option<&'a Path>,
  /// The step's skills folder, read-only.
  pub skills: Option<&'a Path>,
  /// The granted writable folders; they apply only when the step edits files.
  pub writable_folders: &'a [ResolvedFolder],
}

/// The canonicalised folders of one step's scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileScope {
  worktree: PathBuf,
  worktree_writable: bool,
  read_only: Vec<PathBuf>,
  writable_folders: Vec<PathBuf>,
}

impl FileScope {
  /// The scope of `roots`, each canonicalised now; the worktree, code index and skills folders must
  /// exist, a writable folder may still be missing.
  pub fn new(roots: &ScopeRoots<'_>) -> Result<Self, ScopeError> {
    let worktree = canonical_root(roots.worktree)?;
    let read_only = [roots.code_index, roots.skills]
      .into_iter()
      .flatten()
      .map(canonical_root)
      .collect::<Result<_, _>>()?;
    let writes = roots.access == RepositoryAccess::Write;
    let writable_folders = if writes {
      roots
        .writable_folders
        .iter()
        .map(|folder| resolved(folder.path()))
        .collect::<Result<_, _>>()?
    } else {
      Vec::new()
    };
    Ok(Self {
      worktree,
      worktree_writable: writes,
      read_only,
      writable_folders,
    })
  }

  /// The canonical worktree folder.
  pub fn worktree(&self) -> &Path {
    &self.worktree
  }

  /// Where a tool reads `path` (relative to the worktree, or absolute), or why it may not.
  pub fn readable(&self, path: &str) -> Result<PathBuf, ScopeError> {
    let (resolved, form) = self.resolve(path)?;
    let in_writable_folder = self.writable_folders.iter().any(|root| within(&resolved, root));
    let readable = in_writable_folder
      || within(&resolved, &self.worktree)
      || self.read_only.iter().any(|root| within(&resolved, root));
    if form == PathForm::Network && !in_writable_folder {
      return Err(ScopeError::NetworkPath(path.to_owned()));
    }
    if readable {
      Ok(resolved)
    } else {
      Err(ScopeError::NotReadable(resolved))
    }
  }

  /// Where a tool writes `path`, or why it may not. The executor resolves it again just before it
  /// writes, since commands can change the file system in between.
  pub fn writable(&self, path: &str) -> Result<PathBuf, ScopeError> {
    let (resolved, form) = self.resolve(path)?;
    if self.read_only.iter().any(|root| within(&resolved, root)) {
      return Err(ScopeError::Protected(resolved));
    }
    let folder = self.writable_folders.iter().find(|root| within(&resolved, root));
    if form == PathForm::Network && folder.is_none() {
      return Err(ScopeError::NetworkPath(path.to_owned()));
    }
    let root = match folder {
      Some(root) => root,
      None if self.worktree_writable && within(&resolved, &self.worktree) => &self.worktree,
      None => return Err(ScopeError::NotWritable(resolved)),
    };
    if protected(&resolved, root, root == &self.worktree) {
      return Err(ScopeError::Protected(resolved));
    }
    Ok(resolved)
  }

  /// The working folder of a command: `path` inside the worktree, or the worktree when `None`.
  pub fn workdir(&self, path: Option<&str>) -> Result<PathBuf, ScopeError> {
    let Some(path) = path else {
      return Ok(self.worktree.clone());
    };
    let (resolved, form) = self.resolve(path)?;
    if form == PathForm::Local && within(&resolved, &self.worktree) {
      Ok(resolved)
    } else {
      Err(ScopeError::NotReadable(resolved))
    }
  }

  /// Whether a command argument `arg`, taken relative to `workdir` as the command will, names nothing
  /// outside the readable scope: it names no existing path, or one inside the scope. The OS resolves
  /// it, `..` after a link included, exactly as the command would; a network or device path is never
  /// resolved. A drive-relative path (`C:file`) is refused, as the file tools refuse it: it is relative
  /// to a per-drive folder the scope does not know.
  pub fn argument_readable(&self, workdir: &Path, arg: &str) -> bool {
    if arg.contains('\0')
      || platform::names_another_machine(arg)
      || matches!(platform::path_form(arg), Err(ScopeError::DriveRelative(_)))
    {
      return false;
    }
    match fs::canonicalize(workdir.join(arg)) {
      Ok(found) => {
        let found = canonical::plain(found);
        within(&found, &self.worktree)
          || self.read_only.iter().any(|root| within(&found, root))
          || self.writable_folders.iter().any(|root| within(&found, root))
      }
      Err(error) => error.kind() == io::ErrorKind::NotFound,
    }
  }

  /// `path` resolved, and how it was written. A network path outside every writable folder is refused
  /// before the file system is asked: resolving it would contact the server it names.
  fn resolve(&self, path: &str) -> Result<(PathBuf, PathForm), ScopeError> {
    if path.contains('\0') {
      return Err(ScopeError::Nul);
    }
    let form = platform::path_form(path)?;
    let joined = normalize(&self.worktree.join(path));
    if form == PathForm::Network && !self.writable_folders.iter().any(|root| within(&joined, root)) {
      return Err(ScopeError::NetworkPath(path.to_owned()));
    }
    Ok((resolved(&joined)?, form))
  }
}

/// `path`, an existing absolute local path, with its links resolved, for a read outside every scope
/// (`crate::orchester_tools::OutsideReads`). The spellings the file tools refuse are refused here too, and
/// a network path before anything resolves it.
pub fn resolve_local(path: &str) -> Result<PathBuf, ScopeError> {
  if path.contains(' ') {
    return Err(ScopeError::Nul);
  }
  if platform::path_form(path)? == PathForm::Network {
    return Err(ScopeError::NetworkPath(path.to_owned()));
  }
  canonical_root(Path::new(path))
}

/// Whether `path` is spelled as a local path: no network path (`\\server\share\…`), device path or other spelling
/// the file tools refuse. Reads nothing, so it reaches no network.
pub fn is_local_path(path: &str) -> bool {
  matches!(platform::path_form(path), Ok(PathForm::Local))
}

/// Whether `path` is a `.git` entry or lies in one, at any depth, as the platform opens names.
pub fn in_git_folder(path: &Path) -> bool {
  path.components().any(|component| match component {
    Component::Normal(name) => platform::is_named(name, GIT_DIR),
    _ => false,
  })
}

/// A root folder, which must exist, with links resolved; refused before it is resolved when a link on the way leads to
/// another machine ([`crate::permission::link_targets`]).
fn canonical_root(path: &Path) -> Result<PathBuf, ScopeError> {
  crate::permission::link_targets::stay_local(path).map_err(|error| ScopeError::Unresolved {
    path: path.to_path_buf(),
    kind: error.kind(),
  })?;
  fs::canonicalize(path)
    .map(canonical::plain)
    .map_err(|error| ScopeError::Unresolved {
      path: path.to_path_buf(),
      kind: error.kind(),
    })
}

/// `path` with its longest existing ancestor canonicalised.
fn resolved(path: &Path) -> Result<PathBuf, ScopeError> {
  canonical::existing(path)
    .map(|found| found.path)
    .map_err(|failure| ScopeError::Unresolved {
      path: failure.path,
      kind: failure.error.kind(),
    })
}

/// Whether `path` is `root` or lies inside it, component by component, as the platform compares names.
fn within(path: &Path, root: &Path) -> bool {
  let mut components = path.components();
  for expected in root.components() {
    match components.next() {
      Some(actual) if platform::same_name(actual.as_os_str(), expected.as_os_str()) => {}
      _ => return false,
    }
  }
  true
}

/// Whether `path` inside `root` is a `.git` entry or lies in one, or, in the worktree, lies in
/// `.orchester/`, or names a stream of a file or folder (on Windows, `name:stream`), which opens what
/// the name before it opens. Callers check the path with its links and short names resolved as well:
/// `GIT~1` is `.git` only once resolved. The command classifier refuses a shell write there by this same
/// rule.
pub(crate) fn protected(path: &Path, root: &Path, in_worktree: bool) -> bool {
  let below = path.components().skip(root.components().count());
  for (depth, component) in below.enumerate() {
    let Component::Normal(name) = component else {
      continue;
    };
    // A stream of a name (`.git::$INDEX_ALLOCATION`) opens what the name opens.
    if platform::names_stream(name)
      || platform::is_named(name, GIT_DIR)
      || (in_worktree && depth == 0 && platform::is_named(name, STATE_DIR))
    {
      return true;
    }
  }
  false
}
