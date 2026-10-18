//! Where a path a command names lies: in the worktree or a writable folder, in the temporary folder,
//! in the dependency sources of a toolchain ([`super::toolchains`]), a secret-bearing location of the home
//! folder, or elsewhere. Paths are resolved against the folder the command runs in, with `~` as the home
//! folder and links followed as far as they exist.

use std::path::{Component, Path, PathBuf, Prefix};

use super::{glob, toolchains::Toolchains};
use crate::{
  native::scope::protected,
  permission::canonical::{existing, normalize},
};

/// Files directly in the home folder that hold credentials besides its dot files (`_netrc` is
/// curl's `.netrc` on Windows).
const SECRET_HOME_FILES: [&str; 2] = ["_netrc", "ntuser.dat"];

/// The folders of the home folder where applications keep their data, credentials and tokens among
/// it: Windows' `AppData` (`Roaming` and `Local`: the credential and vault folders, `gcloud`, `GitHub
/// CLI`, `pip`, browser and editor profiles, …) and macOS' `Library` (the keychains, `Application
/// Support` with the same profiles, cookies, containers). The command-line tools' own files are dot
/// files and folders of the home folder (`.aws`, `.azure`, `.kube`, `.docker/config.json`,
/// `.config/gh/hosts.yml`, `.config/gcloud`, `.npmrc`, `.yarnrc`, `.pypirc`, `.netrc`,
/// `.cargo/credentials.toml`, `.git-credentials`, `.codex`, `.claude.json`), which are all credential
/// locations too, but for the dependency sources in `.cargo` and `.rustup` ([`super::toolchains`]).
const APPLICATION_DATA: [&str; 2] = ["appdata", "library"];

/// Where commands run and may write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandScope {
  worktree: PathBuf,
  writable: Vec<PathBuf>,
  /// The home folders whose dot files are credential locations, resolved: the user's, and the shell's
  /// `HOME` and `USERPROFILE` where they differ.
  homes: Vec<PathBuf>,
  /// The shell's `HOME` (also `~`) and `USERPROFILE`, as its environment holds them.
  shell_home: Option<PathBuf>,
  profile: Option<PathBuf>,
  temp: Option<PathBuf>,
  /// The Cargo and rustup homes, whose dependency sources are read like the worktree.
  toolchains: Toolchains,
  /// Cargo's target directory as the shell's `CARGO_TARGET_DIR` (or `CARGO_BUILD_TARGET_DIR`) names it, as written.
  cargo_target: Option<PathBuf>,
}

impl CommandScope {
  /// Commands that run in `worktree` and may also write to `writable`, for a user whose home folder is
  /// `home` and whose temporary folder is `temp`. Each is resolved as far as it exists.
  /// The shell's `HOME` and `USERPROFILE` are `home` too; [`Self::with_shell_homes`] sets them apart.
  pub fn new(worktree: &Path, writable: &[PathBuf], home: Option<&Path>, temp: Option<&Path>) -> Self {
    let mut scope = Self {
      worktree: resolved(worktree),
      writable: writable.iter().map(|folder| resolved(folder)).collect(),
      homes: Vec::new(),
      shell_home: home.map(Path::to_path_buf),
      profile: home.map(Path::to_path_buf),
      temp: temp.map(resolved),
      toolchains: Toolchains::default(),
      cargo_target: None,
    };
    scope.add_home(home);
    scope
  }

  /// Adds `home` (as written) to the home folders, with its `.cargo` and `.rustup`.
  fn add_home(&mut self, home: Option<&Path>) {
    let Some(folder) = home.map(resolved).filter(|folder| !self.homes.contains(folder)) else {
      return;
    };
    self.toolchains.add(
      Some(resolved(&folder.join(".cargo"))),
      Some(resolved(&folder.join(".rustup"))),
    );
    self.homes.push(folder);
  }

  /// This scope for a shell whose environment holds `HOME` as `home` and `USERPROFILE` as `profile`
  /// (`None` when unset): `$HOME` and `~` name the first, `$USERPROFILE` the second, and the dot files
  /// of each are credential locations too.
  #[must_use]
  pub fn with_shell_homes(mut self, home: Option<&Path>, profile: Option<&Path>) -> Self {
    self.shell_home = home.map(Path::to_path_buf);
    self.profile = profile.map(Path::to_path_buf);
    self.add_home(home);
    self.add_home(profile);
    self
  }

  /// This scope for a shell whose environment names a Cargo home (`CARGO_HOME`) and a rustup home
  /// (`RUSTUP_HOME`) besides the home folders' `.cargo` and `.rustup`; a relative one is not followed.
  #[must_use]
  pub fn with_toolchain_homes(mut self, cargo: Option<&Path>, rustup: Option<&Path>) -> Self {
    let home = |folder: Option<&Path>| folder.filter(|folder| folder.is_absolute()).map(resolved);
    self.toolchains.add(home(cargo), home(rustup));
    self
  }

  /// This scope for a shell whose environment names Cargo's target directory `folder` (`CARGO_TARGET_DIR`, else
  /// `CARGO_BUILD_TARGET_DIR`), as written: a Cargo build writes there unless it names another
  /// ([`super::build_output`]).
  #[must_use]
  pub fn with_cargo_target_dir(mut self, folder: &Path) -> Self {
    self.cargo_target = Some(folder.to_path_buf());
    self
  }

  /// The scope of the host's user: their home folder and the system's temporary folder, for a shell
  /// with the host's `HOME`, `USERPROFILE`, `CARGO_HOME`, `RUSTUP_HOME` and Cargo's target directory, which
  /// the agent's CLI inherits.
  pub fn on_host(worktree: &Path, writable: &[PathBuf]) -> Self {
    let variable = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let home = variable(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    let mut scope = Self::new(worktree, writable, home.as_deref(), Some(&std::env::temp_dir()))
      .with_shell_homes(variable("HOME").as_deref(), variable("USERPROFILE").as_deref())
      .with_toolchain_homes(variable("CARGO_HOME").as_deref(), variable("RUSTUP_HOME").as_deref());
    scope.cargo_target = variable("CARGO_TARGET_DIR").or_else(|| variable("CARGO_BUILD_TARGET_DIR"));
    scope
  }

  /// Cargo's target directory as the shell's environment names it, as written.
  pub(super) fn cargo_target(&self) -> Option<&Path> {
    self.cargo_target.as_deref()
  }

  /// The Cargo homes, resolved, whose `config.toml` Cargo reads.
  pub(super) fn cargo_homes(&self) -> &[PathBuf] {
    self.toolchains.cargo_homes()
  }

  /// The value of the shell's `HOME` or `USERPROFILE` (`name`); `None` for another name or when unset.
  pub(super) fn home_variable(&self, name: &str) -> Option<&Path> {
    match name {
      "HOME" => self.shell_home.as_deref(),
      "USERPROFILE" => self.profile.as_deref(),
      _ => None,
    }
  }

  pub fn worktree(&self) -> &Path {
    &self.worktree
  }

  /// Whether `folder` is too broad for a rule to cover what lies in it ([`super::broad::is_broad`]), once resolved.
  pub fn is_broad_folder(&self, folder: &Path) -> bool {
    super::broad::is_broad(&resolved(folder), &self.homes)
  }

  /// Whether `path`, as an agent named a file it changed (absolute, or relative to the worktree), names a
  /// file in the worktree once resolved.
  pub fn is_worktree_file(&self, path: &str) -> bool {
    self
      .absolute(&self.worktree, path)
      .is_some_and(|path| resolved(&path).starts_with(&self.worktree))
  }

  /// The paths `word` names from `cwd`, resolved, and where each lies. A word without a glob names one;
  /// a word with a glob names the folder before its first glob component (`src/*.rs` is `src`, `*/x`
  /// the folder `cwd`), a credential location when what the glob matches in it may be one (`~/.s*`),
  /// and each path it matches now ([`glob::expand`]), links followed. `None` when it names the home
  /// folder of another user (`~bob`) or the folders `~+` and `~-`, climbs out of what a glob matched
  /// (`src/*/../..`), or its glob cannot be followed.
  pub(super) fn locate(&self, cwd: &Path, word: &str) -> Option<Vec<(PathBuf, Place)>> {
    let (folder, glob) = before_glob(word)?;
    if !glob {
      let path = resolved(&self.absolute(cwd, word)?);
      return Some(vec![(path.clone(), self.place(&path))]);
    }
    // A glob that cannot be followed names what cannot be told.
    let Ok(matches) = glob::expand(&self.absolute(cwd, word)?) else {
      return None;
    };
    let folder = resolved(&self.absolute(cwd, folder)?);
    let place = match self.place(&folder) {
      Place::Granted => Place::Granted,
      _ if self.may_hold_secrets(&folder) => Place::Secret,
      place => place,
    };
    let mut located = vec![(folder, place)];
    located.extend(matches.iter().map(|path| {
      let path = resolved(path);
      let place = self.place(&path);
      (path, place)
    }));
    Some(located)
  }

  /// The folder `cd word` goes to from `cwd`, resolved: with a glob, the one folder it matches now;
  /// `None` when it matches none or several, or cannot be followed.
  pub(super) fn folder(&self, cwd: &Path, word: &str) -> Option<PathBuf> {
    let path = self.absolute(cwd, word)?;
    if !word.contains(['*', '?', '[']) {
      return Some(resolved(&path));
    }
    match glob::expand(&path).as_deref() {
      Ok([only]) => Some(resolved(only)),
      _ => None,
    }
  }

  /// `word` from `cwd` as written, with `~` as the home folder; `None` for another user's home folder
  /// (`~bob`) and the folders `~+` and `~-`.
  fn absolute(&self, cwd: &Path, word: &str) -> Option<PathBuf> {
    Some(if word == "~" {
      self.shell_home.clone()?
    } else if let Some(rest) = word.strip_prefix("~/").or_else(|| word.strip_prefix("~\\")) {
      self.shell_home.as_ref()?.join(rest)
    } else if word.starts_with('~') {
      return None;
    } else {
      cwd.join(host_path(word)?)
    })
  }

  /// `word` from `cwd` as written, `.` and `..` taken away, no link followed; `None` as for
  /// [`Self::locate`] without a glob.
  pub(super) fn written(&self, cwd: &Path, word: &str) -> Option<PathBuf> {
    self.absolute(cwd, word).map(|path| normalize(&path))
  }

  /// Whether every folder from the worktree or the writable folder `path` (as written) lies in down to
  /// `path` exists now as a folder, none of them a link or junction.
  pub(super) fn plain_folders(&self, path: &Path) -> bool {
    let Some(root) = std::iter::once(&self.worktree)
      .chain(&self.writable)
      .find(|root| path.starts_with(root))
    else {
      return false;
    };
    let mut folder = root.clone();
    path.components().skip(root.components().count()).all(|component| {
      folder.push(component);
      std::fs::symlink_metadata(&folder).is_ok_and(|found| found.is_dir() && !found.file_type().is_symlink())
    })
  }

  /// Where `path` (resolved) lies. The home folder and the folders that hold it are credential
  /// locations too: a program that reads a folder's tree (`grep -r`, `ls -R`, `cp -r`) reads them.
  fn place(&self, path: &Path) -> Place {
    // The worktree may lie in a dot folder of the home folder; what it holds is still the run's.
    if path.starts_with(&self.worktree) || self.writable.iter().any(|folder| path.starts_with(folder)) {
      return Place::Granted;
    }
    if self.homes.iter().any(|home| home.starts_with(path)) || unmapped(path) {
      return Place::Secret;
    }
    match self.toolchains.sources(path) {
      Some(true) => return Place::Sources,
      Some(false) => return Place::Secret,
      None => {}
    }
    if self.secret(path) {
      return Place::Secret;
    }
    if self.temp.as_ref().is_some_and(|temp| path.starts_with(temp)) {
      return Place::Temp;
    }
    Place::Outside
  }

  /// Whether a write to `path` (resolved, in the worktree or a writable folder) is refused as the native
  /// agent's file tools refuse it ([`crate::native::scope`]): a `.git` entry or anything in one, at any
  /// depth, or `.orchester/` at the top of the worktree; or, for a delete, the worktree or a writable
  /// folder itself.
  pub(super) fn write_refused(&self, path: &Path, delete: bool) -> bool {
    // As written and resolved: a short name (`GIT~1`) or a stream spelling opens `.git` only resolved.
    [path.to_path_buf(), resolved(path)].iter().any(|path| {
      let root = if path.starts_with(&self.worktree) {
        Some((&self.worktree, true))
      } else {
        self
          .writable
          .iter()
          .find(|folder| path.starts_with(folder))
          .map(|folder| (folder, false))
      };
      root.is_some_and(|(root, in_worktree)| {
        protected(path, root, in_worktree) || (delete && path.components().count() == root.components().count())
      })
    })
  }

  /// Whether a program written with a folder is installed where the agent cannot have written it: an
  /// absolute path, resolved, outside the worktree, the writable and temporary folders and the credential
  /// locations (`C:\Program Files\PowerShell\7\pwsh.exe`, not `../x/bash`).
  pub(super) fn installed(&self, word: &str) -> bool {
    host_path(word).is_some_and(|path| path.is_absolute() && self.place(&resolved(&path)) == Place::Outside)
  }

  /// Whether `path` lies in the worktree, as written and resolved.
  pub(super) fn in_worktree(&self, path: &Path) -> bool {
    path.starts_with(&self.worktree) && resolved(path).starts_with(&self.worktree)
  }

  /// Whether `path` is the worktree or a writable folder itself.
  pub(super) fn is_root(&self, path: &Path) -> bool {
    std::iter::once(&self.worktree)
      .chain(&self.writable)
      .any(|root| root == path)
  }

  /// Whether a folder's entries may be credential locations: the home folder and the folders above it
  /// (the folders in it are [`Self::secret`] themselves).
  fn may_hold_secrets(&self, folder: &Path) -> bool {
    self.homes.iter().any(|home| home.starts_with(folder))
  }

  /// Whether `path` is a credential location of the home folder: a dot file or folder in it (the agent
  /// CLIs' `.claude.json`, `.claude` and `.codex`, `.ssh`, `.aws`, `.netrc`, `.config` with its tokens,
  /// …), or anything in the applications' data ([`APPLICATION_DATA`]) but the temporary folder, which
  /// Windows keeps in `AppData/Local/Temp`.
  fn secret(&self, path: &Path) -> bool {
    self
      .homes
      .iter()
      .filter_map(|home| path.strip_prefix(home).ok())
      .any(|rest| self.secret_in_home(path, rest))
  }

  /// Whether `path`, which is `rest` inside a home folder, is a credential location of it.
  fn secret_in_home(&self, path: &Path, rest: &Path) -> bool {
    let names: Vec<String> = rest
      .components()
      .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
      .collect();
    match names.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
      [] => false,
      [folder, ..] if APPLICATION_DATA.contains(folder) => {
        !self.temp.as_ref().is_some_and(|temp| path.starts_with(temp))
      }
      [first, ..] => first.starts_with('.') || SECRET_HOME_FILES.contains(first),
    }
  }
}

impl CommandScope {
  /// Whether `path`, absolute, is a credential location for a program that reads it: the home folder's
  /// dot files and folders, the applications' data, and the folders that hold the home folder; never the
  /// worktree or a writable folder. Links are followed as far as they exist.
  pub fn holds_credentials(&self, path: &Path) -> bool {
    self.place(&resolved(path)) == Place::Secret
  }

  /// Whether `path`, absolute, lies in the dependency sources of a toolchain ([`super::toolchains`]), which
  /// are read like the worktree. Links are followed as far as they exist.
  pub fn in_dependency_sources(&self, path: &Path) -> bool {
    self.place(&resolved(path)) == Place::Sources
  }
}

/// Where a path lies, from the most to the least granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
  /// The worktree or an extra writable folder.
  Granted,
  /// The temporary folder.
  Temp,
  /// The dependency sources of a toolchain: read like the worktree, written like anywhere else.
  Sources,
  /// Anywhere else.
  Outside,
  /// A credential location of the home folder: its dot files and folders, and the applications' data
  /// in `AppData` and `Library`.
  Secret,
}

/// Whether `path` starts with a Windows prefix other than a drive letter: a share (`\\host\share`, also
/// `\\localhost\c$`), a `\\?\` or `\\.\` path. Each can name any file of this machine, the home
/// folder's credentials included, under a name the home folder does not match.
fn unmapped(path: &Path) -> bool {
  matches!(
    path.components().next(),
    Some(Component::Prefix(prefix)) if !matches!(prefix.kind(), Prefix::Disk(_))
  )
}

/// `path` with links followed as far as it exists, or as written when it cannot be read.
fn resolved(path: &Path) -> PathBuf {
  existing(path).map_or_else(|_| normalize(path), |found| found.path)
}

/// The part of `word` before its first component with a glob character (empty when that is the first),
/// and whether it had one; `None` when a `..` after the glob leaves what it matched, which the folder
/// before it does not tell.
fn before_glob(word: &str) -> Option<(&str, bool)> {
  let Some(glob) = word.find(['*', '?', '[']) else {
    return Some((word, false));
  };
  let start = word[..glob].rfind(['/', '\\']).map_or(0, |end| end + 1);
  if word[start..].split(['/', '\\']).any(|name| name == "..") {
    return None;
  }
  Some((&word[..start], true))
}

/// `word` as the host reads it: on Windows, Git Bash's `/c/…` is `C:\…`. Any other path rooted
/// without a drive (`/projects/x`, `/tmp/x`) Git Bash places under its own installation, which the
/// classifier does not know: `None`.
fn host_path(word: &str) -> Option<PathBuf> {
  if cfg!(windows) {
    let bytes = word.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && (bytes.len() == 2 || bytes[2] == b'/')
    {
      return Some(PathBuf::from(format!(
        "{}:\\{}",
        char::from(bytes[1]),
        word.get(3..).unwrap_or_default()
      )));
    }
    if word.starts_with('/') && !word.starts_with("//") {
      return None;
    }
  }
  Some(PathBuf::from(word))
}
