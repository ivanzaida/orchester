//! The editor *Resolve in editor* opens a run's worktree and its conflicted files in: the program
//! [`EDITOR_VARIABLE`] names, else VS Code's `code`, else the system's program for folders.

use std::{
  env, io,
  path::{Path, PathBuf},
};

use orchester_core::connection::executable_candidates;
use orchester_exec::{external, search_path::search_path};

use crate::files::Platform;

/// The variable that names the editor *Resolve in editor* starts, a program given the worktree and the conflicted
/// files; a QA copy names a stand-in so that no editor opens on the person's desktop.
pub const EDITOR_VARIABLE: &str = "ORCHESTER_EDITOR";

/// The editor the worktree opens in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Editor {
  /// The program [`EDITOR_VARIABLE`] names.
  Named(PathBuf),
  /// VS Code's `code`, found on the search path.
  VsCode(PathBuf),
  /// The system's default program for folders (Explorer, Finder): the worktree opens, its files do not.
  SystemFolders,
}

impl Editor {
  /// The editor this computer has: the one [`EDITOR_VARIABLE`] names, else VS Code when its `code` is on the search
  /// path, else the system's default for folders; `None` where the platform has none.
  pub fn find() -> Option<Self> {
    if let Some(named) = env::var_os(EDITOR_VARIABLE).filter(|named| !named.is_empty()) {
      return Some(Self::Named(PathBuf::from(named)));
    }
    let path = search_path();
    if let Some(code) = executable_candidates("code", path.as_deref())
      .into_iter()
      .find(|candidate| candidate.is_file())
    {
      return Some(Self::VsCode(code));
    }
    Platform::current().map(|_| Self::SystemFolders)
  }

  /// Whether automation driving the window may start it: only an editor a person named for this purpose.
  pub const fn named(&self) -> bool {
    matches!(self, Self::Named(_))
  }

  /// Opens `worktree` and its `files` (paths from its root); the system's folder program opens the folder alone.
  pub fn open(&self, worktree: &Path, files: &[String]) -> io::Result<()> {
    let program = match self {
      Self::Named(program) | Self::VsCode(program) => program,
      Self::SystemFolders => return external::open_path(worktree),
    };
    let worktree = native(worktree);
    let files: Vec<PathBuf> = files.iter().map(|file| native(&worktree.join(file))).collect();
    let args: Vec<&Path> = std::iter::once(worktree.as_path())
      .chain(files.iter().map(PathBuf::as_path))
      .collect();
    external::start_editor(program, &args)
  }
}

/// `path` with the platform's separators only, as an editor and a person read it: git names files with `/`.
pub fn native(path: &Path) -> PathBuf {
  path.components().collect()
}
