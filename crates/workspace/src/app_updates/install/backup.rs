//! The databases copied aside before an update installs: the application database `orchester.db` with its
//! `orchester.db-wal`, and each open project's `.orchester/orchester.db` with its own, into
//! `backups/<running version>/` of the data folder. A project's copy goes to `projects/<n>-<folder name>/`, and
//! `projects.txt` names the repository each came from. Only the last [`KEEP`] backups stay.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_api::releases::ReleaseVersion;

/// The folder of the data folder backups go to.
pub const BACKUPS_FOLDER: &str = "backups";
/// How many backups stay: this one and the one before.
pub const KEEP: usize = 2;
/// A database's write-ahead log beside it, which holds what is not in the database file yet.
const WAL_SUFFIX: &str = "-wal";
/// Where a project keeps its database, under its repository.
const PROJECT_DATABASE: [&str; 2] = [".orchester", "orchester.db"];
const PROJECTS_FOLDER: &str = "projects";
const PROJECTS_LIST: &str = "projects.txt";

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

/// Copies the database `file` and its write-ahead log, when it has one, into `folder`.
fn copy_database(file: &Path, folder: &Path) -> io::Result<()> {
  let Some(name) = file.file_name() else {
    return Ok(());
  };
  fs::copy(file, folder.join(name))?;
  let mut wal_name = name.to_os_string();
  wal_name.push(WAL_SUFFIX);
  let wal = file.with_file_name(&wal_name);
  if wal.is_file() {
    fs::copy(&wal, folder.join(wal_name))?;
  }
  Ok(())
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
