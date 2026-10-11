//! Exact-pid discovery in the user's folder or an isolated QA profile under .tmp/.

use std::{
  fs,
  io::Read,
  num::NonZeroU32,
  path::{Path, PathBuf},
};

use orchester_core::orchester_tools::AppDiscovery;

use super::AppError;

const MAX_DISCOVERY_BYTES: u64 = 16 * 1024;
const MAX_SEARCH_ENTRIES: usize = 8192;

pub(super) fn find(worktree: &Path, pid: NonZeroU32, app: &str, tool: &str) -> Result<AppDiscovery, AppError> {
  let folder = if app == "orchester-app" {
    "orchester-app-mcp"
  } else if tool.starts_with("lurq_") {
    "lurq/mcp"
  } else {
    "orchester-mcp"
  };
  let name = format!("{pid}.json");
  if let Some(user) = user_data() {
    let path = user.join(folder).join(&name);
    match fs::symlink_metadata(&path) {
      Ok(_) => return read(&path, pid, app),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(_) => return Err(AppError::ReadDiscovery),
    }
  }
  let root = fs::canonicalize(worktree).map_err(|_| AppError::ReadDiscovery)?;
  let matches = qa_files(&root, folder, &name)?;
  match matches.as_slice() {
    [path] => read(path, pid, app),
    [] => Err(AppError::Missing),
    _ => Err(AppError::Ambiguous),
  }
}

#[cfg(windows)]
fn user_data() -> Option<PathBuf> {
  std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn user_data() -> Option<PathBuf> {
  std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn user_data() -> Option<PathBuf> {
  None
}

fn read(path: &Path, pid: NonZeroU32, app: &str) -> Result<AppDiscovery, AppError> {
  let file = fs::File::open(path).map_err(|_| AppError::ReadDiscovery)?;
  let mut bytes = Vec::new();
  file
    .take(MAX_DISCOVERY_BYTES + 1)
    .read_to_end(&mut bytes)
    .map_err(|_| AppError::ReadDiscovery)?;
  if bytes.len() as u64 > MAX_DISCOVERY_BYTES {
    return Err(AppError::ReadDiscovery);
  }
  AppDiscovery::parse(&bytes, pid, app).map_err(AppError::from)
}

fn qa_files(root: &Path, server_folder: &str, name: &str) -> Result<Vec<PathBuf>, AppError> {
  let suffix = Path::new(server_folder).join(name);
  let mut pending = vec![(root.join(".tmp"), 0)];
  let mut matches = Vec::new();
  let mut remaining = MAX_SEARCH_ENTRIES;
  while let Some((folder, depth)) = pending.pop() {
    // Resolve every folder before traversing: a QA profile link must not reach the install or another run.
    let resolved = match fs::canonicalize(&folder) {
      Ok(path) if path.starts_with(root) => path,
      Ok(_) => continue,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(_) => return Err(AppError::ReadDiscovery),
    };
    let entries = fs::read_dir(&resolved).map_err(|_| AppError::ReadDiscovery)?;
    for entry in entries {
      remaining = remaining.checked_sub(1).ok_or(AppError::SearchBound)?;
      let entry = entry.map_err(|_| AppError::ReadDiscovery)?;
      let kind = entry.file_type().map_err(|_| AppError::ReadDiscovery)?;
      if kind.is_symlink() {
        continue;
      }
      if kind.is_file() && entry.path().ends_with(&suffix) {
        let path = fs::canonicalize(entry.path()).map_err(|_| AppError::ReadDiscovery)?;
        if path.starts_with(root) && path.ends_with(&suffix) {
          matches.push(path);
        }
      } else if kind.is_dir() && depth < 12 {
        pending.push((entry.path(), depth + 1));
      }
    }
  }
  Ok(matches)
}
