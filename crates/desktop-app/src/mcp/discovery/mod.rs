//! The discovery file of the app's MCP server: `<pid>.json` in a folder of the user's profile that is
//! the app's own (`orchester-app-mcp/`, see [`platform::user_folder`]), never lurq's `lurq/mcp/` nor
//! the deprecated desktop's `orchester-mcp/`, so tools tell the servers apart. It holds the port, the
//! endpoint and the bearer token, so only the user can read it; it is removed when the server stops.
//! Each process also holds a lock on `<pid>.lock` while it runs: a file whose lock can be taken belongs
//! to a process that ended without removing it (it was killed), and the next server that starts removes
//! it. Adapted from the deprecated desktop's `app_mcp::discovery`, which goes with that crate.

mod platform;

use std::{
  fs::{self, File, TryLockError},
  io::{self, Write},
  num::NonZeroU16,
  path::{Path, PathBuf},
};

use super::{APP_NAME, server::endpoint};

/// The shape of the file; raised when a field changes meaning.
const VERSION: u32 = 1;

/// Where discovery files go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryFolder {
  /// The user's folder for this platform ([`platform::user_folder`]).
  User,
  /// A folder of the caller's choosing, for tests.
  At(PathBuf),
}

impl DiscoveryFolder {
  /// The folder; `None` when this platform has no user folder for it, which the log says.
  pub fn path(&self) -> Option<PathBuf> {
    match self {
      Self::User => platform::user_folder(),
      Self::At(path) => Some(path.clone()),
    }
  }
}

/// This process's discovery file and the lock that marks it as alive; dropping it removes both.
pub struct DiscoveryFile {
  path: PathBuf,
  lock_path: PathBuf,
  /// Held while the process runs; another process taking it knows this one ended.
  lock: Option<File>,
}

/// The path only: the file holds the token.
impl std::fmt::Debug for DiscoveryFile {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("DiscoveryFile")
      .field("path", &self.path)
      .finish()
  }
}

impl DiscoveryFile {
  /// Writes the discovery file of this process in `folder` for a server on `port` with `token`, after
  /// removing the files of processes that ended without removing theirs.
  pub fn write(folder: &Path, port: NonZeroU16, token: &str) -> io::Result<Self> {
    platform::create_private_folder(folder)?;
    remove_stale(folder);
    let pid = std::process::id();
    let lock_path = folder.join(format!("{pid}.lock"));
    // Not truncated: a lock is empty, and a link planted in its place keeps what it points to.
    let lock = File::options()
      .write(true)
      .create(true)
      .truncate(false)
      .open(&lock_path)?;
    lock.try_lock().map_err(|error| match error {
      TryLockError::Error(error) => error,
      TryLockError::WouldBlock => io::Error::other("this process's discovery lock is held already"),
    })?;
    let path = folder.join(format!("{pid}.json"));
    // A file under this pid is left by an earlier process that had it and was killed: this process
    // holds the pid's lock now. It is replaced, as the new file is created only where none is.
    remove(&path);
    let contents = serde_json::json!({
      "version": VERSION,
      "pid": pid,
      "app": APP_NAME,
      "transport": "streamable-http",
      "port": port.get(),
      "url": endpoint(port),
      "token": token,
    });
    let json = serde_json::to_string_pretty(&contents).map_err(io::Error::other)?;
    let mut file = platform::create_private_file(&path)?;
    file.write_all(json.as_bytes())?;
    file.sync_all()?;
    Ok(Self {
      path,
      lock_path,
      lock: Some(lock),
    })
  }

  pub fn path(&self) -> &Path {
    &self.path
  }
}

impl Drop for DiscoveryFile {
  fn drop(&mut self) {
    remove(&self.path);
    // The lock goes with its handle; the file is removed after, so no other process mistakes this one
    // for ended while its discovery file is still there.
    drop(self.lock.take());
    remove(&self.lock_path);
  }
}

/// Removes the discovery files, and their locks, of processes that ended without removing them: those
/// whose lock can be taken, or that have none. Never this process's own. Only discovery files are
/// looked at: a process locks its lock before it writes its discovery file, so a lock without one may
/// belong to a process that is starting.
fn remove_stale(folder: &Path) {
  let entries = match fs::read_dir(folder) {
    Ok(entries) => entries,
    Err(error) => {
      tracing::warn!(%error, folder = %folder.display(), "cannot list the MCP discovery folder");
      return;
    }
  };
  let own = std::process::id();
  let pids = entries
    .filter_map(|entry| {
      // An entry that cannot be read is left alone, like a file not named after a pid.
      entry
        .inspect_err(|error| tracing::warn!(%error, folder = %folder.display(), "cannot read an MCP discovery entry"))
        .ok()
    })
    .filter_map(|entry| named_pid(&entry.path()))
    .filter(|pid| *pid != own);
  for pid in pids {
    let lock_path = folder.join(format!("{pid}.lock"));
    if ended(&lock_path) {
      remove(&folder.join(format!("{pid}.json")));
      remove(&lock_path);
    }
  }
}

/// Whether the process holding `lock_path` ended: its lock can be taken, or it has none.
fn ended(lock_path: &Path) -> bool {
  let lock = match File::options().write(true).open(lock_path) {
    Ok(lock) => lock,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
    Err(error) => {
      tracing::warn!(%error, lock = %lock_path.display(), "cannot open an MCP discovery lock");
      return false;
    }
  };
  match lock.try_lock() {
    Ok(()) => true,
    Err(TryLockError::WouldBlock) => false,
    Err(TryLockError::Error(error)) => {
      tracing::warn!(%error, lock = %lock_path.display(), "cannot check an MCP discovery lock");
      false
    }
  }
}

/// The pid a discovery file is named after: `<digits>.json`.
fn named_pid(path: &Path) -> Option<u32> {
  if path.extension()? != "json" {
    return None;
  }
  let stem = path.file_stem()?.to_str()?;
  if stem.is_empty() || !stem.bytes().all(|byte| byte.is_ascii_digit()) {
    return None;
  }
  stem.parse().ok()
}

fn remove(path: &Path) {
  match fs::remove_file(path) {
    Ok(()) => {}
    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
    Err(error) => tracing::warn!(%error, file = %path.display(), "cannot remove an MCP discovery file"),
  }
}
