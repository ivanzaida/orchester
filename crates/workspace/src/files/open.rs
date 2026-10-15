//! What *Open in editor* opens: the file on disk, with the system's default program
//! (`docs/design/code-viewer.md`, "The page"). Only a version on disk (the checkout or a run's worktree) has
//! one, checked as a read is. A file the system would run rather than show (a program, a script its shell
//! runs, a shortcut) is refused, since the person asked to look at it: by its extension on every platform,
//! and on macOS and Linux also when it may be executed, which `open` would start in a terminal.

use std::path::{Path, PathBuf};

use orchester_core::command::CommandRunner;
use thiserror::Error;

use super::{
  FilePath, FilesError, Version,
  read::resolve_in_folder,
  source::{Source, source},
};

/// Extensions of files that Windows or macOS run, or hand to a program that runs them, when opened: the system's
/// own, and those the usual installers of Python, Perl, Ruby and Git for Windows' Bash register to run.
const RUN_WHEN_OPENED: &[&str] = &[
  "app",
  "application",
  "appref-ms",
  "bash",
  "bat",
  "cmd",
  "com",
  "command",
  "cpl",
  "exe",
  "gadget",
  "hta",
  "inf",
  "ins",
  "isp",
  "jar",
  "js",
  "jse",
  "lnk",
  "msc",
  "msi",
  "msp",
  "mst",
  "pif",
  "pkg",
  "pl",
  "ps1",
  "ps2",
  "psc1",
  "py",
  "pyw",
  "rb",
  "reg",
  "scf",
  "scr",
  "sct",
  "settingcontent-ms",
  "sh",
  "shb",
  "terminal",
  "tool",
  "url",
  "vb",
  "vbe",
  "vbs",
  "ws",
  "wsf",
  "wsh",
];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OpenError {
  #[error(transparent)]
  Files(#[from] FilesError),
  /// A commit's file is not on disk.
  #[error("a commit's file is not on disk")]
  NotOnDisk,
  /// The system would run the file instead of showing it.
  #[error("{0} would be run, not shown")]
  WouldRun(FilePath),
}

/// The file on disk that *Open in editor* opens for `path` in `version`, or why it does not open.
pub fn open_target(
  runner: &dyn CommandRunner,
  repo_root: &Path,
  version: &Version,
  path: &FilePath,
) -> Result<PathBuf, OpenError> {
  let Source::Folder(folder) = source(runner, repo_root, version)? else {
    return Err(OpenError::NotOnDisk);
  };
  let resolved = resolve_in_folder(&folder, path)?;
  if !resolved.is_file() {
    return Err(FilesError::Missing(path.clone()).into());
  }
  if runs_when_opened(&resolved) {
    return Err(OpenError::WouldRun(path.clone()));
  }
  Ok(resolved)
}

/// Whether the system runs the file at `path` rather than showing it when it is opened.
pub fn runs_when_opened(path: &Path) -> bool {
  let by_name = path
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| RUN_WHEN_OPENED.iter().any(|run| run.eq_ignore_ascii_case(extension)));
  by_name || platform::executable(path)
}

#[cfg(unix)]
mod platform {
  use std::{fs, os::unix::fs::PermissionsExt, path::Path};

  /// Any of the execute bits: `open` starts such a file in a terminal. A file whose permissions cannot be
  /// read counts as executable, so it is not opened.
  pub(super) fn executable(path: &Path) -> bool {
    fs::metadata(path).map_or(true, |metadata| metadata.permissions().mode() & 0o111 != 0)
  }
}

#[cfg(windows)]
mod platform {
  use std::path::Path;

  /// Windows decides what runs by the extension alone, which `RUN_WHEN_OPENED` covers.
  pub(super) fn executable(_path: &Path) -> bool {
    false
  }
}
