//! The usual install folders of command-line programs on macOS, in the order they are searched
//! after the inherited `PATH`. Only folders that exist are used.

use std::{
  env,
  ffi::{OsStr, OsString},
  fs,
  path::{Path, PathBuf},
  process::Command as Process,
};

use orchester_core::command::Command;

/// Homebrew on Apple silicon, Homebrew on Intel and the Node.js installer, MacPorts.
const SYSTEM_FOLDERS: [&str; 3] = ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"];
/// In the home folder: the Claude Code and pipx/uv installers, Claude Code's older local install,
/// npm's documented user prefix, Volta, Bun, pnpm and rustup. npm's configured prefix
/// (`~/.npmrc`) and nvm's Node.js versions follow.
const HOME_FOLDERS: [&str; 7] = [
  ".local/bin",
  ".claude/local",
  ".npm-global/bin",
  ".volta/bin",
  ".bun/bin",
  "Library/pnpm",
  ".cargo/bin",
];
const NVM_VERSIONS: &str = ".nvm/versions/node";

pub fn home() -> Option<PathBuf> {
  env::var_os("HOME").map(PathBuf::from).filter(|home| home.is_absolute())
}

/// The system folders, then those in `home`, that exist.
pub fn usual_folders(home: Option<&Path>) -> Vec<PathBuf> {
  let mut folders: Vec<PathBuf> = SYSTEM_FOLDERS.iter().map(PathBuf::from).collect();
  if let Some(home) = home {
    folders.extend(home_folders(home));
  }
  folders.retain(|folder| folder.is_dir());
  folders
}

/// The folders in `home` where CLIs are installed, in search order, whether they exist or not:
/// [`HOME_FOLDERS`], the `bin` of npm's `prefix` in `~/.npmrc`, then nvm's Node.js versions,
/// newest first.
pub fn home_folders(home: &Path) -> Vec<PathBuf> {
  let mut folders: Vec<PathBuf> = HOME_FOLDERS.iter().map(|folder| home.join(folder)).collect();
  if let Some(prefix) = npm_prefix(home) {
    folders.push(prefix.join("bin"));
  }
  folders.extend(nvm_folders(home));
  folders
}

/// `path` followed by each of `folders` it does not contain yet.
pub fn extend(path: Option<&OsStr>, folders: &[PathBuf]) -> OsString {
  let mut entries: Vec<PathBuf> = path.map(|path| env::split_paths(path).collect()).unwrap_or_default();
  for folder in folders {
    if !entries.contains(folder) {
      entries.push(folder.clone());
    }
  }
  env::join_paths(entries).unwrap_or_else(|error| {
    // A folder with `:` in its name cannot be on a PATH; keep the PATH as it was.
    tracing::warn!(%error, "cannot extend the PATH");
    path.map(OsStr::to_os_string).unwrap_or_default()
  })
}

/// Gives the process of `command` the search path as its `PATH`: the `PATH` the command sets or
/// inherits, then the usual folders, then the folder of `program` (the file that starts), so a
/// CLI finds the Node.js installed beside it.
pub(crate) fn apply(process: &mut Process, command: &Command, program: &Path) {
  let set = command
    .env()
    .iter()
    .rev()
    .find(|(key, _)| key == "PATH")
    .map(|(_, value)| OsString::from(value));
  let path = set.or_else(|| command.inherits_env().then(|| env::var_os("PATH")).flatten());
  let mut folders = usual_folders(home().as_deref());
  if let Some(folder) = program.parent().filter(|folder| folder.is_absolute()) {
    folders.push(folder.to_path_buf());
  }
  process.env("PATH", extend(path.as_deref(), &folders));
}

/// The `prefix` npm installs global packages under, when `~/.npmrc` sets one.
fn npm_prefix(home: &Path) -> Option<PathBuf> {
  // Without a readable `~/.npmrc` npm uses its default prefix, which the system folders cover.
  let npmrc = fs::read_to_string(home.join(".npmrc")).ok()?;
  let value = npmrc.lines().find_map(|line| {
    let (key, value) = line.split_once('=')?;
    (key.trim() == "prefix").then(|| value.trim())
  })?;
  let expanded = value
    .strip_prefix("~/")
    .or_else(|| value.strip_prefix("${HOME}/"))
    .or_else(|| value.strip_prefix("$HOME/"))
    .map_or_else(|| PathBuf::from(value), |rest| home.join(rest));
  expanded.is_absolute().then_some(expanded)
}

/// The `bin` folders of nvm's Node.js versions (`vX.Y.Z`), newest version first.
fn nvm_folders(home: &Path) -> Vec<PathBuf> {
  // No readable versions folder: nvm is not installed.
  let Ok(entries) = fs::read_dir(home.join(NVM_VERSIONS)) else {
    return Vec::new();
  };
  let mut versions: Vec<((u64, u64, u64), PathBuf)> = entries
    // An entry that cannot be read is not a version to search.
    .filter_map(Result::ok)
    .filter_map(|entry| Some((node_version(entry.file_name().to_str()?)?, entry.path().join("bin"))))
    .collect();
  versions.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
  versions.into_iter().map(|(_, folder)| folder).collect()
}

fn node_version(name: &str) -> Option<(u64, u64, u64)> {
  let mut parts = name.strip_prefix('v')?.splitn(3, '.').map(str::parse::<u64>);
  Some((parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?))
}
