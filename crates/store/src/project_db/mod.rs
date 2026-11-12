mod activity;
mod agent_files;
mod always_allow;
mod approval_modes;
mod approval_records;
mod approval_rules;
mod archive_stops;
mod attachments;
mod auto_mode;
mod automatic_status;
mod automation_drafts;
mod base_branch;
mod base_notices;
mod blobs;
mod chat_decisions;
mod checks;
mod ci_fixes;
mod code_index;
mod completions;
mod connections;
mod consistent;
mod context_uses;
mod decision_connections;
mod decision_review_usage;
mod decision_test_history;
mod decision_tests;
mod default_pipeline;
mod disk_guard;
mod environment;
mod estimates;
mod feedback;
mod import;
mod invocation_accounts;
mod invocation_connections;
mod invocation_prompts;
mod invocation_providers;
mod invocation_records;
mod joining;
mod limits;
mod local_archive;
mod local_integration;
mod local_rename;
mod mcp_servers;
mod memory;
mod merge_attempts;
mod merge_queue;
mod merges;
mod message_records;
mod messages;
mod modules;
mod native;
mod new_resources;
mod notice_records;
mod orchestrator;
mod orchestrator_compactions;
mod orchestrator_fork;
mod orchestrator_leads;
mod orchestrator_plans;
mod orchestrator_queue;
mod orchestrator_reports;
mod orchestrator_rewind;
mod orchestrator_sent;
mod orchestrator_settings;
mod orchestrator_subscriptions;
mod orchestrator_switches;
mod orchestrator_titles;
mod orchestrator_usage;
mod orchestrator_waits;
mod orchestrator_watch_summaries;
mod organization;
mod organized_drafts;
mod owned_records;
mod permission_events;
mod pipeline_publishes;
mod plan_limits;
mod plans;
mod project_rules;
mod pull_request_attempts;
mod pull_request_diffs;
mod pull_requests;
mod question_records;
mod questions;
mod queue_holds;
mod read_only;
mod rebases;
mod removal_records;
mod removals;
mod repository;
mod repository_import;
mod request_answers;
mod resolutions;
mod resources;
mod review_threads;
mod reviewer_usage;
mod revision_diffs;
mod run_connections;
mod run_ends;
mod run_inputs;
mod run_models;
mod run_progress;
mod run_waits;
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
mod side_questions;
mod sources;
mod step_outputs;
mod step_progress;
mod step_records;
mod tags;
mod task_children;
mod task_deletion;
mod task_effort;
mod task_groups;
mod task_identities;
mod task_snapshots;
mod tasks;
mod team_deliveries;
mod team_fill_ins;
mod team_leaving;
mod team_operations;
mod team_runs;
mod team_starts;
mod todo_lists;
mod tool_approvals;
mod usage;
mod used_versions;
mod version_publishers;
mod workspace_records;
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
  approval_records::ApprovalRecord,
  approval_rules::DecisionCounts,
  archive_stops::{ArchiveStop, Archived},
  attachments::{AttachmentOwner, attachment_file},
  auto_mode::AutoModeSave,
  automatic_status::{AutomaticStatus, NotSent, UnsentStatus},
  chat_decisions::KeptAction,
  decision_test_history::RecordedDecisionReview,
  estimates::AutomationPoints,
  invocation_records::{ActivityRecord, InvocationRecord, InvocationStart},
  joining::{JoinedKey, ReturnedTask},
  message_records::MessageRecord,
  notice_records::NoticeRecord,
  orchestrator::{ChatListing, ChatTurn},
  orchestrator_fork::ChatFork,
  orchestrator_plans::ApprovedPlan,
  orchestrator_queue::StartedQueued,
  orchestrator_reports::KeptReport,
  orchestrator_rewind::ChatRewind,
  orchestrator_subscriptions::{KeptSubscription, WatchStart},
  orchestrator_titles::ChatDeletion,
  orchestrator_waits::KeptWait,
  owned_records::{
    AlwaysAllowAnswer, AlwaysAllowRecord, MemoryAnswer, MemoryRecord, ModelRequirement, RuleAnswer, RuleRecord,
  },
  question_records::QuestionRecord,
  read_only::ReadOnlyProjectDb,
  removal_records::RemovalRecord,
  resolutions::ResolutionStart,
  resources::ResourceUpdate,
  run_ends::EndedRun,
  run_inputs::AcceptedInputs,
  run_waits::{KeptRunWait, RunWaitEnd, RunWaitEnded},
  runs::{RunBranch, RunLogRecord},
  shared_events::{SharedEvent, SharedSnapshot},
  shared_members::{ServerAdmin, SharedMember},
  shared_modules::{CachedModule, CachedModuleChange},
  shared_tasks::{CachedClaim, CachedRunStatus, CachedTask, SharedPerson, SharedTaskInfo},
  shared_versions::{CachedVersion, ReferenceTarget, ResourceSharing},
  sharing::SharingLink,
  step_records::StepRecord,
  task_deletion::TaskDeletionFacts,
  team_deliveries::{DeliveryMerge, DeliveryReport, TeamDelivery, TeamDeliveryStage},
  team_fill_ins::ConnectionPick,
  team_operations::{NewTeamOperation, TeamOperation, TeamOperationState, TeamSettlement},
  team_runs::{AcceptedWork, NewWorkReport, TeamFence, TeamRun, WorkReportEntry, WorkReportState},
  team_starts::{TeamStart, TeamStartStage},
  workspace_records::{WarmWorkspaceRecord, WorkspaceWritten},
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

  /// The schema version right before `migration`, wherever the list placed it: a test rewinds to it with
  /// [`ProjectDb::create_at_version`] and then lets the migrations from `migration` on run over its rows, however many
  /// are appended or inserted after it.
  #[cfg(feature = "test-support")]
  pub fn version_before(migration: ProjectMigration) -> u32 {
    let sql = schema::named(migration);
    let position = schema::MIGRATIONS
      .iter()
      .position(|listed| *listed == sql)
      .expect("a named migration is in the list");
    u32::try_from(position).expect("fewer than 2^32 migrations")
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
  /// The rebuild of `resources` for the team reviewer's kind.
  TeamReviewer,
  /// The team tasks' pipeline slug turned into the pipeline's id.
  TeamPipelineIds,
  /// Local profiles choosing what reached their agents before.
  ProfileGiven,
  /// The outbox of the team's configuration dropped.
  TeamOutbox,
  /// A local project's drafts of its published profiles, instructions, skills and tools dropped.
  LocalDrafts,
  /// Pipeline drafts deleted, and every pipeline with no version with what names it.
  PipelineDrafts,
  /// The `permissions` rebuild of tool approvals: a request permissions decided.
  PermittedDecisions,
  /// The automation rebuild of tool approvals, which follows the `permissions` one.
  AutomationAnswers,
  /// Auto mode's settings and the rebuild of tool approvals for its answers.
  AutoMode,
  /// The project's base branch moved to its tasks and modules.
  BaseSources,
  /// The disk guard: the free space a project keeps.
  DiskGuard,
  /// Approval modes, which replace auto mode's settings.
  ApprovalModes,
  /// The decision reviewer's JSON settings.
  DecisionReviewer,
  /// Tasks' identities on a team server.
  TaskIdentities,
  /// The attempts table rebuilt for a run returned to a step after it ended; the run resolutions table follows it.
  ReturnedAfterEnd,
  /// What an archive stop names: the resource, its kind and who archived it.
  ArchiveStopsArchived,
}
