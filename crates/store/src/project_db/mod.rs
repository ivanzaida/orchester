mod activity;
mod agent_files;
mod always_allow;
mod approval_modes;
mod approval_rules;
mod attachments;
mod auto_mode;
mod automatic_status;
mod automation_drafts;
mod base_branch;
mod base_notices;
mod blobs;
mod chat_decisions;
mod checks;
mod code_index;
mod completions;
mod connections;
mod consistent;
mod default_pipeline;
mod environment;
mod estimates;
mod feedback;
mod import;
mod invocation_accounts;
mod invocation_connections;
mod invocation_prompts;
mod invocation_providers;
mod joining;
mod limits;
mod local_integration;
mod mcp_servers;
mod memory;
mod merge_queue;
mod merges;
mod messages;
mod modules;
mod native;
mod orchestrator;
mod orchestrator_queue;
mod orchestrator_reports;
mod orchestrator_sent;
mod orchestrator_usage;
mod orchestrator_waits;
mod organization;
mod organized_drafts;
mod plan_limits;
mod plans;
mod project_rules;
mod pull_request_attempts;
mod pull_request_diffs;
mod pull_requests;
mod questions;
mod queue_holds;
mod read_only;
mod rebases;
mod reference_bindings;
mod removals;
mod repository;
mod repository_import;
mod request_answers;
mod resolutions;
mod resources;
mod reviewer_usage;
mod revision_diffs;
mod run_connections;
mod run_ends;
mod runner_failures;
mod runs;
mod schema;
mod secrets;
mod service_needs;
mod shared_events;
mod shared_members;
mod shared_modules;
mod shared_publishes;
mod shared_tasks;
mod shared_versions;
mod sharing;
mod sources;
mod step_outputs;
mod step_progress;
mod tags;
mod task_deletion;
mod task_effort;
mod task_groups;
mod task_snapshots;
mod tasks;
mod tool_approvals;
mod usage;
mod used_versions;
mod workspaces;
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
  attachments::{AttachmentOwner, attachment_file},
  auto_mode::AutoModeSave,
  automatic_status::{AutomaticStatus, NotSent, UnsentStatus},
  chat_decisions::KeptAction,
  estimates::AutomationPoints,
  joining::{JoinedKey, ReturnedTask},
  orchestrator::{ChatListing, ChatTurn},
  orchestrator_queue::StartedQueued,
  orchestrator_reports::KeptReport,
  orchestrator_waits::KeptWait,
  read_only::ReadOnlyProjectDb,
  reference_bindings::{Bound, BoundBy, BoundReference, ReferenceBinding},
  resolutions::ResolutionStart,
  resources::ResourceUpdate,
  run_ends::EndedRun,
  runs::RunBranch,
  shared_events::{SharedEvent, SharedSnapshot},
  shared_members::SharedMember,
  shared_modules::{CachedModule, CachedModuleChange},
  shared_publishes::{PublishState, WaitingPublish},
  shared_tasks::{CachedClaim, CachedRunStatus, CachedTask, SharedPerson, SharedTaskInfo},
  shared_versions::{CachedVersion, NamingVersion, ReferenceTarget, ResourceSharing, VersionReference},
  sharing::SharingLink,
  task_deletion::TaskDeletionFacts,
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

/// Excludes `.orchester/` from git in the repository at `repo_root`, as [`ProjectDb::create`] does: for a project's
/// folder made a repository after the project was.
pub fn exclude_project_data(repo_root: &Path) -> Result<(), StoreError> {
  git_exclude::exclude(repo_root, EXCLUDE_PATTERN)
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

  /// Applies `migration` again to the database of `connection`, which a test shaped as the schema
  /// before it: a named migration, rather than every one after a rewound version, so later ones
  /// that cannot run twice stay out of the test.
  #[cfg(feature = "test-support")]
  pub fn apply_migration_for_test(connection: &Connection, migration: ProjectMigration) -> Result<(), StoreError> {
    connection.execute_batch(schema::named(migration))?;
    Ok(())
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

/// A project schema migration a test applies again by name ([`ProjectDb::apply_migration_for_test`]).
#[cfg(feature = "test-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectMigration {
  /// The local integration journal, its retained content and its process record.
  LocalIntegration,
  /// Retained content that also keeps editor drafts.
  EditorDrafts,
}
