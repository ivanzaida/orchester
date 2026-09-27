mod activity;
mod blobs;
mod connections;
mod invocation_prompts;
mod limits;
mod plans;
mod pull_request_diffs;
mod pull_requests;
mod repository;
mod resources;
mod revision_diffs;
mod runs;
mod schema;
mod secrets;
mod step_outputs;
mod tasks;

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  name::Name,
  project::{Project, ProjectKey},
};
use rusqlite::{Connection, params};

use crate::{
  StoreError, git_exclude, migrate,
  rows::{corrupt, parse},
};

const DIRECTORY: &str = ".orchester";
const DATABASE: &str = "orchester.db";
const EXCLUDE_PATTERN: &str = "/.orchester/";

/// The project database in `<repo>/.orchester/orchester.db`.
pub struct ProjectDb {
  connection: Connection,
  project: Project,
  /// `<repo>/.orchester`, where the database and the blob files live.
  directory: PathBuf,
}

impl ProjectDb {
  /// Initializes Orchester in the repository at `repo_root` and excludes `.orchester/` from git.
  pub fn create(repo_root: &Path, project: Project) -> Result<Self, StoreError> {
    let database = database_path(repo_root);
    if database.exists() {
      return Err(StoreError::AlreadyInitialized(repo_root.to_path_buf()));
    }
    git_exclude::exclude(repo_root, EXCLUDE_PATTERN)?;
    let directory = repo_root.join(DIRECTORY);
    fs::create_dir_all(&directory).map_err(StoreError::io(&directory))?;
    let connection = migrate::open(&database, schema::MIGRATIONS)?;
    connection.execute(
      "INSERT INTO project (id, key, name) VALUES (?1, ?2, ?3)",
      params![
        project.id().to_string(),
        project.key().as_str(),
        project.name().as_str()
      ],
    )?;
    Ok(Self {
      connection,
      project,
      directory,
    })
  }

  /// Opens an initialized repository, applying pending migrations.
  pub fn open(repo_root: &Path) -> Result<Self, StoreError> {
    let database = database_path(repo_root);
    if !database.is_file() {
      return Err(StoreError::NotInitialized(repo_root.to_path_buf()));
    }
    let connection = migrate::open(&database, schema::MIGRATIONS)?;
    let project = load_project(&connection)?;
    Ok(Self {
      connection,
      project,
      directory: repo_root.join(DIRECTORY),
    })
  }

  pub const fn project(&self) -> &Project {
    &self.project
  }
}

fn database_path(repo_root: &Path) -> PathBuf {
  repo_root.join(DIRECTORY).join(DATABASE)
}

fn load_project(connection: &Connection) -> Result<Project, StoreError> {
  let (id, key, name): (String, String, String) =
    connection.query_row("SELECT id, key, name FROM project", [], |row| {
      Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
  Ok(Project::new(
    parse("project id", &id)?,
    ProjectKey::new(&key).map_err(|error| corrupt("project key", error))?,
    Name::new(&name).map_err(|error| corrupt("project name", error))?,
  ))
}
