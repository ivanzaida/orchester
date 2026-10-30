//! The folders of the search path, each with where it came from, and the lookup of a program in
//! them that names the folders it searched when the program is not there.

use std::{
  env,
  ffi::{OsStr, OsString},
  path::{Path, PathBuf},
};

use orchester_core::connection::{candidates_in, searched_folder};

/// Where a folder of the search path came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderSource {
  /// The `PATH` this process inherited or the command sets.
  Inherited,
  /// The `PATH` the user's login shell builds (macOS, read at start and when a person asks again).
  LoginShell,
  /// One of the usual install folders of command-line programs (macOS).
  InstallFolder,
  /// The folder of the program that starts, so a CLI finds the Node.js installed beside it (macOS).
  ProgramFolder,
}

/// What a search of the search path looked at, so what it says of a program not found is true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchBasis {
  /// The `PATH` alone: Windows.
  Path,
  /// macOS, after a read of this login shell's `PATH` succeeded.
  LoginShell { shell: String },
  /// macOS, with no login shell's `PATH`: started from a terminal, or the read failed.
  UsualFolders,
}

/// `path` as a person reads it: on `home`, written with `~`.
pub fn shown_folder(path: &Path, home: Option<&Path>) -> String {
  match home.and_then(|home| path.strip_prefix(home).ok()) {
    Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
    Some(rest) => format!("~/{}", rest.display()),
    None => path.display().to_string(),
  }
}

/// One folder of the search path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchFolder {
  path: PathBuf,
  source: FolderSource,
}

impl SearchFolder {
  pub fn path(&self) -> &Path {
    &self.path
  }

  pub const fn source(&self) -> FolderSource {
    self.source
  }

  /// The folder as a person reads it: on macOS the home folder is `~` (a Windows path stays whole).
  pub fn shown(&self) -> String {
    #[cfg(target_os = "macos")]
    let home = super::macos::home();
    #[cfg(not(target_os = "macos"))]
    let home: Option<PathBuf> = None;
    shown_folder(&self.path, home.as_deref())
  }
}

/// What looking up a program on the search path found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramLookup {
  /// The copies found, in search order.
  Found(Vec<PathBuf>),
  /// No copy: the folders looked in, in search order.
  NotFound { searched: Vec<SearchFolder> },
}

/// The search path for programs: its folders in search order, each with its source.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchPath {
  folders: Vec<SearchFolder>,
}

impl SearchPath {
  /// Every entry of `inherited` in its order, then each of `login_shell` and then of `install`
  /// that is not on it yet. A folder that cannot be on a `PATH` (one with the separator in its
  /// name) is left out.
  pub fn merge(inherited: Option<&OsStr>, login_shell: &[PathBuf], install: &[PathBuf]) -> Self {
    let mut search = Self::default();
    for entry in inherited.map(env::split_paths).into_iter().flatten() {
      search.push(entry, FolderSource::Inherited);
    }
    for folder in login_shell {
      search.add(folder, FolderSource::LoginShell);
    }
    for folder in install {
      search.add(folder, FolderSource::InstallFolder);
    }
    search
  }

  /// This process's search path: its `PATH`.
  #[cfg(not(target_os = "macos"))]
  pub fn current() -> Self {
    Self::merge(env::var_os("PATH").as_deref(), &[], &[])
  }

  /// This process's search path: its `PATH`, then the login shell's, then the usual install folders.
  #[cfg(target_os = "macos")]
  pub fn current() -> Self {
    use super::macos;

    let install = macos::usual_folders(macos::home().as_deref());
    Self::merge(env::var_os("PATH").as_deref(), &macos::login_shell_folders(), &install)
  }

  /// This search path with `folder`, the started program's own, last when it is not on it yet.
  #[cfg(target_os = "macos")]
  #[must_use]
  pub fn with_program_folder(mut self, folder: &Path) -> Self {
    self.add(folder, FolderSource::ProgramFolder);
    self
  }

  pub fn folders(&self) -> &[SearchFolder] {
    &self.folders
  }

  /// The folders as the value of a `PATH` variable.
  pub fn joined(&self) -> OsString {
    env::join_paths(self.folders.iter().map(SearchFolder::path))
      .expect("the search path holds only folders a PATH can hold: `push` leaves out the others")
  }

  /// The copies of `program` (see `ConnectionKind::program_name`) in the searched folders, in
  /// search order, or the folders searched when there is none. A folder that is not searched
  /// (`orchester_core::connection::searched_folder`) is not listed.
  pub fn lookup(&self, program: &str) -> ProgramLookup {
    let mut searched: Vec<SearchFolder> = Vec::new();
    let mut found: Vec<PathBuf> = Vec::new();
    for folder in &self.folders {
      if !searched_folder(&folder.path) || searched.iter().any(|seen| seen.path == folder.path) {
        continue;
      }
      searched.push(folder.clone());
      for candidate in candidates_in(program, &folder.path) {
        if candidate.is_file() && !found.contains(&candidate) {
          found.push(candidate);
        }
      }
    }
    if found.is_empty() {
      ProgramLookup::NotFound { searched }
    } else {
      ProgramLookup::Found(found)
    }
  }

  /// Adds `folder` at the end unless it is on the search path already.
  fn add(&mut self, folder: &Path, source: FolderSource) {
    if !self.folders.iter().any(|known| known.path == folder) {
      self.push(folder.to_path_buf(), source);
    }
  }

  fn push(&mut self, path: PathBuf, source: FolderSource) {
    if let Err(error) = env::join_paths([&path]) {
      tracing::warn!(%error, folder = %path.display(), "a folder that cannot be on a PATH is not searched");
      return;
    }
    self.folders.push(SearchFolder { path, source });
  }
}
