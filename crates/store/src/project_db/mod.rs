mod activity;
mod agent_files;
mod always_allow;
mod approval_rules;
mod auto_mode;
mod automatic_status;
mod blobs;
mod checks;
mod code_index;
mod connections;
mod consistent;
mod environment;
mod import;
mod invocation_accounts;
mod invocation_prompts;
mod joining;
mod limits;
mod mcp_servers;
mod memory;
mod merges;
mod messages;
mod native;
mod orchestrator;
mod orchestrator_usage;
mod organization;
mod organized_drafts;
mod plan_limits;
mod plans;
mod pull_request_diffs;
mod pull_requests;
mod questions;
mod queue_holds;
mod read_only;
mod repository;
mod repository_import;
mod resources;
mod reviewer_usage;
mod revision_diffs;
mod run_connections;
mod runs;
mod schema;
mod secrets;
mod shared_tasks;
mod sharing;
mod sources;
mod step_outputs;
mod step_progress;
mod tags;
mod task_groups;
mod tasks;
mod tool_approvals;
mod usage;
mod worktree_removals;
mod write_transaction;

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  name::Name,
  project::{Project, ProjectKey},
};
use rusqlite::{Connection, params};

pub use self::{
  approval_rules::DecisionCounts,
  auto_mode::AutoModeSave,
  automatic_status::{AutomaticStatus, NotSent, UnsentStatus},
  joining::{JoinedKey, ReturnedTask},
  orchestrator::{ChatListing, ChatTurn},
  read_only::ReadOnlyProjectDb,
  resources::ResourceUpdate,
  shared_tasks::{CachedClaim, CachedRunStatus, CachedTask, SharedPerson, SharedTaskInfo},
  sharing::SharingLink,
};
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

  /// Creates the database of `repo_root` at schema `version`, with only the first `version`
  /// migrations applied, and returns it empty: a migration test fills it with the rows that
  /// version wrote, then [`ProjectDb::open`] applies the rest.
  #[cfg(feature = "test-support")]
  pub fn create_at_version(repo_root: &Path, version: u32) -> Result<Connection, StoreError> {
    let Some(migrations) = schema::MIGRATIONS.get(..version as usize) else {
      let supported = u32::try_from(schema::MIGRATIONS.len()).expect("fewer than 2^32 migrations");
      return Err(StoreError::SchemaTooNew {
        found: version,
        supported,
      });
    };
    let database = database_path(repo_root);
    if database.exists() {
      return Err(StoreError::AlreadyInitialized(repo_root.to_path_buf()));
    }
    let directory = repo_root.join(DIRECTORY);
    fs::create_dir_all(&directory).map_err(StoreError::io(&directory))?;
    migrate::open(&database, migrations)
  }

  /// Replaces the timeout with a fixture's lock-wait observer for deterministic concurrency tests.
  #[cfg(feature = "test-support")]
  pub fn set_busy_handler_for_test(&self, callback: fn(i32) -> bool) -> Result<(), StoreError> {
    self.connection.busy_handler(Some(callback))?;
    Ok(())
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
