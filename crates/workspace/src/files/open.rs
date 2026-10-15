//! What *Open in editor* opens (the coordinator's decision of 2026-10-15): only text — source, configuration,
//! markup, documentation and data files by their extension ([`super::text_types`]), or a file without one whose
//! content is text — and only a version on disk (the checkout or a run's worktree), checked as a read is. On
//! macOS a file opens in the default text editor (`open -t`), never with the program its kind names; on Windows
//! with its default program, which for some text kinds runs the file, so those stay closed there. Everything
//! else is refused with its reason. The decision is [`open_with`], a pure function of the platform and what is
//! known of the file; [`open_target`] gathers that at the moment of opening, from the file's own name (a link is
//! refused) and its canonical path (on macOS an execute bit is refused), so what opens is what was checked.

use std::{
  fs,
  io::Read as _,
  path::{Path, PathBuf},
};

use orchester_core::command::CommandRunner;
use thiserror::Error;

use super::{
  FilePath, FilesError, Version,
  read::resolve_in_folder,
  source::{Source, source},
  text_types,
};

/// Bytes looked at for a NUL to tell text, as reads do.
const TEXT_PROBE: usize = 8 * 1024;

/// The platforms *Open in editor* knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
  Windows,
  MacOs,
}

impl Platform {
  /// The platform this build runs on; `None` where opening a file is not supported yet.
  pub const fn current() -> Option<Self> {
    if cfg!(target_os = "windows") {
      Some(Self::Windows)
    } else if cfg!(target_os = "macos") {
      Some(Self::MacOs)
    } else {
      None
    }
  }
}

/// How a file opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenWith {
  /// The system's default program for the file (Windows).
  DefaultProgram,
  /// The system's default text editor (macOS `open -t`).
  TextEditor,
}

/// Why a file does not open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
  /// Not a text or code file.
  NotText,
  /// The platform would run it instead of showing it.
  WouldRun,
  /// It is a link.
  Link,
  /// It may be executed (an execute bit, on macOS).
  Executable,
}

/// What is known of a file when it is decided how it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenFacts<'a> {
  /// The file's name, its last path part.
  pub name: &'a str,
  /// What it holds is text (no NUL in its first bytes).
  pub text: bool,
  pub link: bool,
  /// An execute bit is set (Unix).
  pub executable: bool,
}

/// How `file` opens on `platform`, or why it does not.
pub fn open_with(platform: Platform, file: &OpenFacts<'_>) -> Result<OpenWith, Refusal> {
  if file.link {
    return Err(Refusal::Link);
  }
  let extension = Path::new(file.name)
    .extension()
    .and_then(|extension| extension.to_str());
  let text = match extension {
    Some(extension) => text_types::is_text(extension) && file.text,
    None => file.text,
  };
  if !text {
    return Err(Refusal::NotText);
  }
  match platform {
    Platform::MacOs if file.executable => Err(Refusal::Executable),
    Platform::MacOs => Ok(OpenWith::TextEditor),
    Platform::Windows if extension.is_some_and(text_types::runs_on_windows) => Err(Refusal::WouldRun),
    Platform::Windows => Ok(OpenWith::DefaultProgram),
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OpenError {
  #[error(transparent)]
  Files(#[from] FilesError),
  /// A commit's file is not on disk.
  #[error("a commit's file is not on disk")]
  NotOnDisk,
  #[error("{0} is not opened: {1:?}")]
  Refused(FilePath, Refusal),
  /// This platform has no way to open a file yet.
  #[error("opening a file is not supported on this platform yet")]
  Unsupported,
}

/// The canonical path *Open in editor* opens for `path` in `version`, and how; checked now, just before it opens.
pub fn open_target(
  runner: &dyn CommandRunner,
  repo_root: &Path,
  version: &Version,
  path: &FilePath,
) -> Result<(PathBuf, OpenWith), OpenError> {
  let platform = Platform::current().ok_or(OpenError::Unsupported)?;
  let Source::Folder(folder) = source(runner, repo_root, version)? else {
    return Err(OpenError::NotOnDisk);
  };
  let unreadable = |error: std::io::Error| match error.kind() {
    std::io::ErrorKind::NotFound => OpenError::from(FilesError::Missing(path.clone())),
    kind => FilesError::Unreadable { path: path.clone(), kind }.into(),
  };
  let link = fs::symlink_metadata(folder.join(path.as_str()))
    .map_err(unreadable)?
    .file_type()
    .is_symlink();
  let resolved = resolve_in_folder(&folder, path)?;
  let metadata = fs::symlink_metadata(&resolved).map_err(unreadable)?;
  if !metadata.is_file() {
    return Err(FilesError::Missing(path.clone()).into());
  }
  let facts = OpenFacts {
    name: path.name(),
    text: holds_text(&resolved).map_err(unreadable)?,
    link: link || metadata.file_type().is_symlink(),
    executable: platform::executable(&metadata),
  };
  let with = open_with(platform, &facts).map_err(|refusal| OpenError::Refused(path.clone(), refusal))?;
  Ok((resolved, with))
}

/// Whether the file at `path` holds text: no NUL in its first bytes.
fn holds_text(path: &Path) -> std::io::Result<bool> {
  let mut probe = Vec::with_capacity(TEXT_PROBE);
  fs::File::open(path)?
    .take(u64::try_from(TEXT_PROBE).unwrap_or(u64::MAX))
    .read_to_end(&mut probe)?;
  Ok(!probe.contains(&0))
}

#[cfg(unix)]
mod platform {
  use std::{fs::Metadata, os::unix::fs::PermissionsExt};

  /// Any of the execute bits.
  pub(super) fn executable(metadata: &Metadata) -> bool {
    metadata.permissions().mode() & 0o111 != 0
  }
}

#[cfg(windows)]
mod platform {
  use std::fs::Metadata;

  /// Windows has no execute bit: what runs is decided by the extension, which [`super::open_with`] weighs.
  pub(super) fn executable(_metadata: &Metadata) -> bool {
    false
  }
}
