//! Copying the ripgrep found into the project's tools folder, `rg-<hash>/` by content. Every call
//! stages its copy in a folder of its own and renames it into place, so agents starting together
//! never share or delete a half-made copy. The content hash is kept per file (path, size and change
//! time) for the life of the process, so an agent start does not read the program again. Staging
//! folders left by a copy that stopped are removed after an hour. Each time a copy is handed to an
//! agent its `selected` marker is written again; another copy is removed once it was last handed out
//! a day ago, so agents that started with it have ended, also when the version switches back and
//! forth.

use std::{
  collections::HashMap,
  fs, io,
  path::{Path, PathBuf},
  sync::{
    LazyLock,
    atomic::{AtomicU64, Ordering},
  },
  time::{Duration, SystemTime},
};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

use super::{Found, PROGRAM, ProvideError};

/// Hex digits of the content hash in a copy's folder name.
const HASH_DIGITS: usize = 16;
const COPY: &str = "rg-";
const STAGING: &str = ".rg-";
/// A staging folder this old belongs to a copy that stopped.
const STALE_STAGING: Duration = Duration::from_secs(60 * 60);
/// A copy last handed to an agent this long ago is no longer run.
const UNUSED_COPY: Duration = Duration::from_secs(24 * 60 * 60);
/// The file in a copy's folder written each time the copy is handed to an agent.
const SELECTED: &str = "selected";

/// Numbers the staging folders of this process.
static STAGED: AtomicU64 = AtomicU64::new(0);
/// A program file as it was hashed: its path, size and change time.
type Hashed = (PathBuf, u64, SystemTime);
/// Content hashes of the programs hashed so far.
static HASHES: LazyLock<Mutex<HashMap<Hashed, String>>> = LazyLock::new(Mutex::default);

/// The folder in `tools` that holds a copy of `found`, copied now unless a copy of the same content
/// is there already. Agents get this folder first on their `PATH`.
pub fn provide(found: &Found, tools: &Path) -> Result<PathBuf, ProvideError> {
  let read = |source| ProvideError::Read {
    path: found.path.clone(),
    source,
  };
  let metadata = fs::metadata(&found.path).map_err(read)?;
  let hash = hash(&found.path, &metadata).map_err(read)?;
  let folder = tools.join(format!("{COPY}{hash}"));
  let copy = folder.join(PROGRAM);
  if !fs::metadata(&copy).is_ok_and(|placed| placed.len() == metadata.len()) {
    place(&found.path, &folder, (tools, &hash))?;
  }
  let marker = folder.join(SELECTED);
  if let Err(error) = fs::write(&marker, "") {
    // The copy is still given; without the marker it may go a day after it was placed.
    tracing::debug!(%error, marker = %marker.display(), "cannot mark the ripgrep copy as selected");
  }
  remove_others(tools, &folder);
  Ok(folder)
}

/// The content hash of the program at `path`, read unless this process hashed it before.
fn hash(path: &Path, metadata: &fs::Metadata) -> io::Result<String> {
  let key = (path.to_path_buf(), metadata.len(), metadata.modified()?);
  if let Some(hash) = HASHES.lock().get(&key) {
    return Ok(hash.clone());
  }
  let content = fs::read(path)?;
  let hash: String = Sha256::digest(&content)
    .iter()
    .take(HASH_DIGITS / 2)
    .map(|byte| format!("{byte:02x}"))
    .collect();
  HASHES.lock().insert(key, hash.clone());
  Ok(hash)
}

/// Copies `program` into `folder` through a staging folder of this call's own.
fn place(program: &Path, folder: &Path, (tools, hash): (&Path, &str)) -> Result<(), ProvideError> {
  let failed = |path: &Path| {
    let path = path.to_path_buf();
    move |source| ProvideError::Copy { path, source }
  };
  let number = STAGED.fetch_add(1, Ordering::Relaxed);
  let staging = tools.join(format!("{STAGING}{hash}-{}-{number}", std::process::id()));
  fs::create_dir_all(&staging).map_err(failed(&staging))?;
  let staged = staging.join(PROGRAM);
  fs::copy(program, &staged).map_err(failed(&staged))?;
  executable(&staged).map_err(failed(&staged))?;
  match fs::rename(&staging, folder) {
    Ok(()) => Ok(()),
    // Another start placed the same copy first.
    Err(_) if folder.join(PROGRAM).is_file() => remove_folder(&staging).map_err(failed(&staging)),
    Err(source) => {
      if let Err(error) = remove_folder(&staging) {
        tracing::debug!(%error, folder = %staging.display(), "cannot remove a ripgrep staging folder");
      }
      Err(ProvideError::Copy {
        path: folder.to_path_buf(),
        source,
      })
    }
  }
}

/// Lets everyone run the copy on macOS; on Windows every file can be run.
#[cfg(unix)]
fn executable(file: &Path) -> io::Result<()> {
  use std::os::unix::fs::PermissionsExt;
  fs::set_permissions(file, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn executable(_file: &Path) -> io::Result<()> {
  Ok(())
}

/// Removes staging folders a stopped copy left, and the copies in `tools` other than `current` last
/// handed to an agent [`UNUSED_COPY`] ago (placed then, for one without a marker). A copy an agent
/// still runs cannot be removed on Windows and is tried again next time.
fn remove_others(tools: &Path, current: &Path) {
  let entries = match fs::read_dir(tools) {
    Ok(entries) => entries,
    Err(error) => {
      tracing::debug!(%error, folder = %tools.display(), "cannot look for old ripgrep copies");
      return;
    }
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let name = entry.file_name();
    let name = name.to_string_lossy();
    let stale = if name.starts_with(STAGING) {
      age(&path).is_some_and(|age| age > STALE_STAGING)
    } else {
      name.starts_with(COPY)
        && path != current
        && age(&path.join(SELECTED))
          .or_else(|| age(&path))
          .is_some_and(|age| age > UNUSED_COPY)
    };
    if stale && let Err(error) = remove_folder(&path) {
      tracing::debug!(%error, folder = %path.display(), "cannot remove an old ripgrep copy yet");
    }
  }
}

/// How long ago `path` last changed; `None` when that cannot be read.
fn age(path: &Path) -> Option<Duration> {
  match fs::metadata(path).and_then(|metadata| metadata.modified()) {
    Ok(modified) => modified.elapsed().ok(),
    Err(error) => {
      tracing::debug!(%error, path = %path.display(), "cannot read when a ripgrep file changed");
      None
    }
  }
}

fn remove_folder(folder: &Path) -> io::Result<()> {
  match fs::remove_dir_all(folder) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
    _ => Ok(()),
  }
}
