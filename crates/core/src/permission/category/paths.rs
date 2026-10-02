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

  /// `word`, a path as the command names it from `cwd`, resolved, and where it lies. A word with a glob
  /// is the folder before its first glob component (`src/*.rs` is `src`, `*/x` the folder `cwd`), which
  /// is a credential location when what the glob matches in it may be one (`~/.s*`). `None` when it
  /// names the home folder of another user (`~bob`) or the folders `~+` and `~-`.
  pub(super) fn locate(&self, cwd: &Path, word: &str) -> Option<(PathBuf, Place)> {
    let (word, glob) = before_glob(word);
    let path = if word == "~" {
      self.home.clone()?
    } else if let Some(rest) = word.strip_prefix("~/").or_else(|| word.strip_prefix("~\\")) {
      self.home.as_ref()?.join(rest)
    } else if word.starts_with('~') {
      return None;
    } else {
      cwd.join(host_path(word))
    };
    let path = resolved(&path);
    let place = match self.place(&path) {
      Place::Granted => Place::Granted,
      _ if glob && self.may_hold_secrets(&path) => Place::Secret,
      place => place,
    };
    Some((path, place))
  }

  /// `word` resolved from `cwd`, without its place.
  pub(super) fn path(&self, cwd: &Path, word: &str) -> Option<PathBuf> {
    self.locate(cwd, word).map(|(path, _)| path)
  }

  /// Where `path` (resolved) lies. The home folder and the folders that hold it are credential
  /// locations too: a program that reads a folder's tree (`grep -r`, `ls -R`, `cp -r`) reads them.
  fn place(&self, path: &Path) -> Place {
    // The worktree may lie in a dot folder of the home folder; what it holds is still the run's.
    if path.starts_with(&self.worktree) || self.writable.iter().any(|folder| path.starts_with(folder)) {
      return Place::Granted;
    }
    if self.secret(path) || self.home.as_ref().is_some_and(|home| home.starts_with(path)) {
      return Place::Secret;
    }
    if self.temp.as_ref().is_some_and(|temp| path.starts_with(temp)) {
      return Place::Temp;
    }
    Place::Outside
  }

  /// Whether a folder's entries may be credential locations: the home folder and the folders above
  /// it, `AppData` down to its `Microsoft` folders, and `Library`.
  fn may_hold_secrets(&self, folder: &Path) -> bool {
    let Some(home) = self.home.as_ref() else {
      return false;
    };
    if home.starts_with(folder) {
      return true;
    }
    let Ok(rest) = folder.strip_prefix(home) else {
      return false;
    };
    let names: Vec<String> = rest
      .components()
      .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
      .collect();
    matches!(
      names.iter().map(String::as_str).collect::<Vec<_>>().as_slice(),
      ["appdata"] | ["appdata", _] | ["appdata", _, "microsoft"] | ["library"]
    )
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

/// The part of `word` before its first component with a glob character (empty when that is the first),
/// and whether it had one.
fn before_glob(word: &str) -> (&str, bool) {
  let Some(glob) = word.find(['*', '?', '[']) else {
    return (word, false);
  };
  let folder = word[..glob].rfind(['/', '\\']).map_or("", |end| &word[..=end]);
  (folder, true)
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
