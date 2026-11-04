//! The databases copied aside before an update installs: the application database `orchester.db`, and each open
//! project's `.orchester/orchester.db`, into `backups/<running version>/` of the data folder. A project's copy goes to
//! `projects/<n>-<folder name>/`, and `projects.txt` names the repository each came from. Only the last [`KEEP`]
//! backups stay.
//!
//! Each copy is taken through SQLite (`VACUUM INTO`), so it is one database file of its own that holds what is still
//! in the write-ahead log too. The app's connections hold these databases open while the copy is taken, and on macOS
//! closing any descriptor of a file drops every record lock the process holds on it, SQLite's own included (ORC-630):
//! copying the files with `fs::copy` let a runner take itself for the first or the last connection and cut or delete
//! the shared memory and the log the app still used. A connection of the same SQLite in this process shares the app's
//! descriptors and locks instead (ORC-631).

use std::{
  fs, io,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_api::releases::ReleaseVersion;
use rusqlite::{Connection, OpenFlags};

/// The folder of the data folder backups go to.
pub const BACKUPS_FOLDER: &str = "backups";
/// How many backups stay: this one and the one before.
pub const KEEP: usize = 2;
/// Where a project keeps its database, under its repository.
const PROJECT_DATABASE: [&str; 2] = [".orchester", "orchester.db"];
const PROJECTS_FOLDER: &str = "projects";
const PROJECTS_LIST: &str = "projects.txt";
/// How long a copy waits while another connection holds a lock it needs, as a runner that recovers the log does.
const BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// What one backup copies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Databases {
  /// The application database's file; `None` for one only in memory.
  pub app: Option<PathBuf>,
  /// The open projects' repositories.
  pub projects: Vec<PathBuf>,
}

/// Copies `databases` to `backups/<version>/` of `data_folder`, replacing an earlier backup of that version, and
/// removes all but the last [`KEEP`] backups. A project without a database is left out. Blocking; the folder of the
/// backup.
pub fn back_up(data_folder: &Path, version: &ReleaseVersion, databases: &Databases) -> io::Result<PathBuf> {
  let backups = data_folder.join(BACKUPS_FOLDER);
  let folder = backups.join(version.as_str());
  if folder.exists() {
    fs::remove_dir_all(&folder)?;
  }
  fs::create_dir_all(&folder)?;
  if let Some(app) = &databases.app {
    copy_database(app, &folder)?;
  }
  let mut list = String::new();
  for (index, repo) in databases.projects.iter().enumerate() {
    let database = PROJECT_DATABASE.iter().fold(repo.clone(), |path, part| path.join(part));
    if !database.is_file() {
      continue;
    }
    let name = repo
      .file_name()
      .map_or_else(|| "project".into(), |name| name.to_string_lossy());
    let into = folder.join(PROJECTS_FOLDER).join(format!("{}-{name}", index + 1));
    fs::create_dir_all(&into)?;
    copy_database(&database, &into)?;
    list.push_str(&format!("{}-{name}\t{}\n", index + 1, repo.display()));
  }
  if !list.is_empty() {
    fs::write(folder.join(PROJECTS_LIST), list)?;
  }
  prune(&backups)?;
  Ok(folder)
}

/// The backups in `data_folder`, oldest first: the folders named by a version.
pub fn backups(data_folder: &Path) -> io::Result<Vec<ReleaseVersion>> {
  let entries = match fs::read_dir(data_folder.join(BACKUPS_FOLDER)) {
    Ok(entries) => entries,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(error) => return Err(error),
  };
  let mut versions = Vec::new();
  for entry in entries {
    let entry = entry?;
    if entry.file_type()?.is_dir()
      && let Ok(version) = ReleaseVersion::parse(&entry.file_name().to_string_lossy())
    {
      versions.push(version);
    }
  }
  versions.sort();
  Ok(versions)
}

/// Copies the database `file`, with what its write-ahead log holds, into `folder` under the same name, through a
/// read-only connection: it neither creates the database nor checkpoints or removes its log when it closes.
fn copy_database(file: &Path, folder: &Path) -> io::Result<()> {
  let Some(name) = file.file_name() else {
    return Ok(());
  };
  let copy = folder.join(name);
  let into = copy.to_str().ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("the backup's path is not UTF-8: {}", copy.display()),
    )
  })?;
  let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
  let source = Connection::open_with_flags(file, flags).map_err(sqlite_error)?;
  source.busy_timeout(BUSY_TIMEOUT).map_err(sqlite_error)?;
  source.execute("VACUUM INTO ?1", [into]).map_err(sqlite_error)?;
  source.close().map_err(|(_, error)| sqlite_error(error))
}

fn sqlite_error(error: rusqlite::Error) -> io::Error {
  io::Error::other(error)
}

/// Removes the backups older than the last [`KEEP`], by version.
fn prune(backups: &Path) -> io::Result<()> {
  let Some(data_folder) = backups.parent() else {
    return Ok(());
  };
  let versions = self::backups(data_folder)?;
  let old = versions.len().saturating_sub(KEEP);
  for version in &versions[..old] {
    fs::remove_dir_all(backups.join(version.as_str()))?;
  }
  Ok(())
}
