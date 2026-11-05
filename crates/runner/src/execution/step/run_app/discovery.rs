//! Exact-pid discovery in the user's folder or a QA copy's folder under `.tmp/qa/<task>/`. A QA copy of the app
//! writes its file to the folder `ORCHESTER_APP_MCP_DISCOVERY` names, which `tools/qa-app.ps1` sets to
//! `.tmp/qa/<task>/mcp/`; a QA profile, a copy whose user data folder is `.tmp/qa/<task>/profile/`, keeps it in that
//! folder's `orchester-app-mcp/`. Only those exact paths are read: agents keep build output and scratch checkouts in
//! `.tmp/`, so it is never walked. A QA copy the script named `orchester-app-qa` is reached through its own task
//! folder's file only ([`qa_copy`]).

use std::{
  fs,
  io::Read,
  num::NonZeroU32,
  path::{Component, Path, PathBuf},
};

use orchester_core::orchester_tools::AppDiscovery;

use super::AppError;

const MAX_DISCOVERY_BYTES: u64 = 16 * 1024;
pub(super) const MAX_QA_ENTRIES: usize = 1024;
/// The app's name in its discovery file (`orchester-app`'s `mcp::APP_NAME`).
const APP: &str = "orchester-app";
/// The app's discovery folder in a user data folder.
const FOLDER: &str = "orchester-app-mcp";

pub(super) fn find(worktree: &Path, pid: NonZeroU32) -> Result<AppDiscovery, AppError> {
  let name = format!("{pid}.json");
  if let Some(user) = user_data() {
    let path = user.join(FOLDER).join(&name);
    match fs::symlink_metadata(&path) {
      Ok(_) => return read(&path, pid),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(_) => return Err(AppError::ReadDiscovery),
    }
  }
  let root = fs::canonicalize(worktree).map_err(|_| AppError::ReadDiscovery)?;
  let layouts = [
    Path::new("profile").join(FOLDER).join(&name),
    Path::new("mcp").join(&name),
  ];
  let matches = qa_files(&root, &layouts)?;
  match matches.as_slice() {
    [path] => read(path, pid),
    [] => Err(AppError::Missing),
    _ => Err(AppError::Ambiguous),
  }
}

/// The discovery of the QA copy `executable` (pid `pid`) that `tools/qa-app.ps1` started from the worktree: it runs
/// from `.tmp/qa/<task>/bin/` there, and its file is that task folder's `mcp/<pid>.json`, never one of another folder.
/// Refused when the copy runs from anywhere else. Both paths are compared with links resolved.
pub(super) fn qa_copy(worktree: &Path, executable: &Path, pid: NonZeroU32) -> Result<AppDiscovery, AppError> {
  let root = fs::canonicalize(worktree).map_err(|_| AppError::ReadDiscovery)?;
  let executable = fs::canonicalize(executable).map_err(|_| AppError::QaCopyPlace)?;
  let place: Vec<_> = executable
    .strip_prefix(&root)
    .map_err(|_| AppError::QaCopyPlace)?
    .components()
    .collect();
  let task = match place.as_slice() {
    [
      Component::Normal(tmp),
      Component::Normal(qa),
      Component::Normal(task),
      Component::Normal(bin),
      _,
    ] if tmp.eq_ignore_ascii_case(".tmp") && qa.eq_ignore_ascii_case("qa") && bin.eq_ignore_ascii_case("bin") => task,
    _ => return Err(AppError::QaCopyPlace),
  };
  let file = root
    .join(".tmp")
    .join("qa")
    .join(task)
    .join("mcp")
    .join(format!("{pid}.json"));
  read(&qa_file(&file)?.ok_or(AppError::Missing)?, pid)
}

/// The user's data folder, which holds the apps' discovery folders and the QA slots.
#[cfg(windows)]
pub(in crate::execution::step) fn user_data() -> Option<PathBuf> {
  std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
pub(in crate::execution::step) fn user_data() -> Option<PathBuf> {
  std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(in crate::execution::step) fn user_data() -> Option<PathBuf> {
  None
}

pub(super) fn read(path: &Path, pid: NonZeroU32) -> Result<AppDiscovery, AppError> {
  let file = fs::File::open(path).map_err(|_| AppError::ReadDiscovery)?;
  let mut bytes = Vec::new();
  file
    .take(MAX_DISCOVERY_BYTES + 1)
    .read_to_end(&mut bytes)
    .map_err(|_| AppError::ReadDiscovery)?;
  if bytes.len() as u64 > MAX_DISCOVERY_BYTES {
    return Err(AppError::ReadDiscovery);
  }
  AppDiscovery::parse(&bytes, pid, APP).map_err(AppError::from)
}

/// The files `.tmp/qa/<task>/<layout>` of `root` for every task folder, reading `.tmp/qa/` one level only.
fn qa_files(root: &Path, layouts: &[PathBuf]) -> Result<Vec<PathBuf>, AppError> {
  // Resolve `.tmp/qa` first: a link there must not reach the install or another run.
  let qa = match fs::canonicalize(root.join(".tmp/qa")) {
    Ok(path) if path.starts_with(root) => path,
    Ok(_) => return Ok(Vec::new()),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(_) => return Err(AppError::ReadDiscovery),
  };
  let mut matches = Vec::new();
  for (index, entry) in fs::read_dir(&qa).map_err(|_| AppError::ReadDiscovery)?.enumerate() {
    if index == MAX_QA_ENTRIES {
      return Err(AppError::SearchBound { folder: qa });
    }
    let entry = entry.map_err(|_| AppError::ReadDiscovery)?;
    // A link or junction as a task folder is skipped, not followed.
    if !entry.file_type().map_err(|_| AppError::ReadDiscovery)?.is_dir() {
      continue;
    }
    let task = qa.join(entry.file_name());
    for layout in layouts {
      if let Some(path) = qa_file(&task.join(layout))? {
        matches.push(path);
      }
    }
  }
  Ok(matches)
}

/// `path` when it is a regular file reached without any link on the way from its task folder.
pub(super) fn qa_file(path: &Path) -> Result<Option<PathBuf>, AppError> {
  match fs::symlink_metadata(path) {
    Ok(metadata) if metadata.is_file() => {}
    Ok(_) => return Ok(None),
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
    Err(_) => return Err(AppError::ReadDiscovery),
  }
  let resolved = fs::canonicalize(path).map_err(|_| AppError::ReadDiscovery)?;
  Ok((resolved == path).then_some(resolved))
}
