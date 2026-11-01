//! The macOS side of the search path: the `PATH` of the user's login shell, read as the app starts
//! and again when a person asks, and the usual install folders of command-line programs, in the
//! order they are searched after the inherited `PATH`. Only install folders that exist are used.

use std::{
  env,
  ffi::OsString,
  fs,
  io::{self, IsTerminal},
  path::{Path, PathBuf},
  process::Command as Process,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
};

use orchester_core::command::Command;
use parking_lot::Mutex;

use super::{
  SearchBasis, SearchPath,
  login_shell::{self, LoginShellPath},
};

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
/// The login shell when `$SHELL` names none: macOS's default since Catalina.
const DEFAULT_SHELL: &str = "/bin/zsh";

/// What the reads found of the login shell's `PATH`; `None` until the first read ended, which holds the lock so
/// whoever needs it waits for it.
static LOGIN_SHELL: Mutex<Option<LoginShellPath>> = Mutex::new(None);
/// Whether this process reads the login shell's `PATH` ([`start_reading`]).
static READING: AtomicBool = AtomicBool::new(false);

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

/// Starts reading the login shell's `PATH` on a thread of its own, so the app's start does not wait
/// for the user's shell start-up files.
pub(super) fn start_reading() {
  // Started from a terminal, the inherited PATH is the shell's already; and an interactive shell
  // in a process group of its own would be stopped as soon as it touched that terminal.
  if io::stdin().is_terminal() {
    tracing::info!("started from a terminal: the login shell's PATH is not read");
    return;
  }
  READING.store(true, Ordering::Release);
  let reading = thread::Builder::new().name("login-shell-path".into()).spawn(|| {
    login_shell_folders();
  });
  if let Err(error) = reading {
    tracing::warn!(%error, "cannot read the login shell's PATH in the background; the first program search reads it");
  }
}

/// The folders of the login shell's `PATH`: none in a process that does not read it ([`start_reading`]),
/// otherwise as last read, waiting for the first read, which takes at most [`login_shell::TIMEOUT`].
pub fn login_shell_folders() -> Arc<[PathBuf]> {
  known().map_or_else(Arc::default, |known| Arc::clone(known.folders()))
}

/// Reads the login shell's `PATH` again, without holding up the programs that start meanwhile; nothing in a process
/// that does not read it ([`start_reading`]).
pub(super) fn read_again() {
  if !READING.load(Ordering::Acquire) {
    return;
  }
  let known = LOGIN_SHELL.lock().clone().unwrap_or_default();
  let read = read_login_shell(known);
  *LOGIN_SHELL.lock() = Some(read);
}

/// What a search now can say it looked at, waiting for the first read as [`login_shell_folders`] does.
pub(super) fn basis() -> SearchBasis {
  known().map_or(SearchBasis::UsualFolders, |known| known.basis())
}

/// What is known of the login shell's `PATH`; `None` in a process that does not read it.
fn known() -> Option<LoginShellPath> {
  if !READING.load(Ordering::Acquire) {
    return None;
  }
  let mut known = LOGIN_SHELL.lock();
  Some(
    known
      .get_or_insert_with(|| read_login_shell(LoginShellPath::default()))
      .clone(),
  )
}

/// Gives the process of `command` the search path as its `PATH`: the `PATH` the command sets or
/// inherits, then the login shell's and the usual folders, then the folder of `program` (the file
/// that starts), so a CLI finds the Node.js installed beside it.
pub(crate) fn apply(process: &mut Process, command: &Command, program: &Path) {
  let set = command
    .env()
    .iter()
    .rev()
    .find(|(key, _)| key == "PATH")
    .map(|(_, value)| OsString::from(value));
  let path = set.or_else(|| command.inherits_env().then(|| env::var_os("PATH")).flatten());
  let install = usual_folders(home().as_deref());
  let mut search = SearchPath::merge(path.as_deref(), &login_shell_folders(), &install);
  if let Some(folder) = program.parent().filter(|folder| folder.is_absolute()) {
    search = search.with_program_folder(folder);
  }
  process.env("PATH", search.joined());
}

/// `known` after reading the login shell's `PATH`; the log says when the shell cannot be read.
fn read_login_shell(known: LoginShellPath) -> LoginShellPath {
  let shell = env::var_os("SHELL")
    .map(PathBuf::from)
    .filter(|shell| shell.is_absolute())
    .unwrap_or_else(|| PathBuf::from(DEFAULT_SHELL));
  let read = login_shell::read(&shell, login_shell::TIMEOUT);
  match &read {
    Ok(folders) => {
      tracing::info!(folders = folders.len(), shell = %shell.display(), "read the login shell's PATH");
    }
    Err(error) => {
      tracing::warn!(%error, shell = %shell.display(), "cannot read the login shell's PATH; programs are searched without it");
    }
  }
  known.after_read(read, &shell)
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
