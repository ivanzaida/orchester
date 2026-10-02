//! Where a path a command names lies: in the worktree or a writable folder, in the temporary folder,
//! a secret-bearing location of the home folder, or elsewhere. Paths are resolved against the folder
//! the command runs in, with `~` as the home folder and links followed as far as they exist.

use std::path::{Path, PathBuf};

use crate::permission::canonical::{existing, normalize};

/// Files directly in the home folder that hold credentials besides its dot files (`_netrc` is
/// curl's `.netrc` on Windows).
const SECRET_HOME_FILES: [&str; 2] = ["_netrc", "ntuser.dat"];

/// Under `AppData/Roaming/Microsoft` and `AppData/Local/Microsoft`, the folders Windows keeps
/// credentials and their keys in.
const SECRET_APPDATA: [&str; 3] = ["credentials", "vault", "protect"];

/// Where commands run and may write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandScope {
  worktree: PathBuf,
  writable: Vec<PathBuf>,
  home: Option<PathBuf>,
  temp: Option<PathBuf>,
}

impl CommandScope {
  /// Commands that run in `worktree` and may also write to `writable`, for a user whose home folder is
  /// `home` and whose temporary folder is `temp`. Each is resolved as far as it exists.
  pub fn new(worktree: &Path, writable: &[PathBuf], home: Option<&Path>, temp: Option<&Path>) -> Self {
    Self {
      worktree: resolved(worktree),
      writable: writable.iter().map(|folder| resolved(folder)).collect(),
      home: home.map(resolved),
      temp: temp.map(resolved),
    }
  }

  /// The scope of the host's user: their home folder and the system's temporary folder.
  pub fn on_host(worktree: &Path, writable: &[PathBuf]) -> Self {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    Self::new(worktree, writable, home.as_deref(), Some(&std::env::temp_dir()))
  }

  pub fn worktree(&self) -> &Path {
    &self.worktree
  }

  /// `word`, a path as the command names it from `cwd`, resolved; `None` when it holds a glob that
  /// leaves even its folder open (`*/x`), or names the home folder of another user (`~bob`).
  pub(super) fn path(&self, cwd: &Path, word: &str) -> Option<PathBuf> {
    let word = before_glob(word)?;
    let path = if word == "~" {
      self.home.clone()?
    } else if let Some(rest) = word.strip_prefix("~/").or_else(|| word.strip_prefix("~\\")) {
      self.home.as_ref()?.join(rest)
    } else if word.starts_with('~') {
      return None;
    } else {
      cwd.join(host_path(word))
    };
    Some(resolved(&path))
  }

  /// Where `path` (resolved) lies.
  pub(super) fn place(&self, path: &Path) -> Place {
    // The worktree may lie in a dot folder of the home folder; what it holds is still the run's.
    if path.starts_with(&self.worktree) || self.writable.iter().any(|folder| path.starts_with(folder)) {
      return Place::Granted;
    }
    if self.secret(path) {
      return Place::Secret;
    }
    if self.temp.as_ref().is_some_and(|temp| path.starts_with(temp)) {
      return Place::Temp;
    }
    Place::Outside
  }

  /// Whether `path` is a credential location of the home folder: a dot file or folder in it (the agent
  /// CLIs' `.claude.json`, `.claude` and `.codex`, `.ssh`, `.aws`, `.netrc`, `.config` with its tokens,
  /// …), the Windows credential folders, or the macOS keychains.
  fn secret(&self, path: &Path) -> bool {
    let Some(rest) = self.home.as_ref().and_then(|home| path.strip_prefix(home).ok()) else {
      return false;
    };
    let names: Vec<String> = rest
      .components()
      .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
      .collect();
    match names.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
      [] => false,
      ["appdata", _, "microsoft", folder, ..] => SECRET_APPDATA.contains(folder),
      ["library", "keychains", ..] => true,
      [first, ..] => first.starts_with('.') || SECRET_HOME_FILES.contains(first),
    }
  }
}

/// Where a path lies, from the most to the least granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
  /// The worktree or an extra writable folder.
  Granted,
  /// The temporary folder.
  Temp,
  /// Anywhere else.
  Outside,
  /// A credential location of the home folder: its dot files and folders, the Windows credential
  /// folders and the macOS keychains.
  Secret,
}

/// `path` with links followed as far as it exists, or as written when it cannot be read.
fn resolved(path: &Path) -> PathBuf {
  existing(path).map_or_else(|_| normalize(path), |found| found.path)
}

/// The part of `word` before its first component with a glob character; `None` when that is all of it
/// and the folder itself is a glob.
fn before_glob(word: &str) -> Option<&str> {
  let Some(glob) = word.find(['*', '?', '[']) else {
    return Some(word);
  };
  let folder_end = word[..glob].rfind(['/', '\\'])?;
  Some(&word[..=folder_end])
}

/// `word` as the host reads it: on Windows, Git Bash's `/c/…` is `C:\…`.
fn host_path(word: &str) -> PathBuf {
  if cfg!(windows) {
    let bytes = word.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && (bytes.len() == 2 || bytes[2] == b'/')
    {
      return PathBuf::from(format!(
        "{}:\\{}",
        char::from(bytes[1]),
        word.get(3..).unwrap_or_default()
      ));
    }
  }
  PathBuf::from(word)
}
