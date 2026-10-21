//! Project database migrations. Append only: never edit a migration that has shipped. Later
//! migrations live in modules of their own, named after what they add.

mod agent_accounts;
mod agent_files;
mod agent_slots;
mod api_providers;
mod approval_choices;
mod attachments;
mod auto_mode;
mod automation_answers;
mod base_notices;
mod code_index;
mod command_approvals;
mod conversation;
mod creation_snapshots;
mod decision_authors;
mod editor_drafts;
mod import;
mod invocation_permissions;
mod joining;
mod list;
mod local_integration;
mod mcp;
mod memory;
mod modules;
mod native_commands;
mod native_conversations;
mod optional_daily_budget;
mod optional_limits;
mod orchestrator;
mod orchestrator_reports;
mod orchestrator_run_ends;
mod orchestrator_sent;
mod orchestrator_waits;
mod organization;
mod permitted_decisions;
mod plan_decided_by;
mod plan_limits;
mod project_approval_rules;
mod pull_request_syncs;
mod qa_slots;
mod questions;
mod queue_holds;
mod recovery;
mod returned_after_end;
mod reviewer_usage;
mod run_rebases;
mod run_resolutions;
mod runner_failures;
mod shared_members;
mod sharing;
mod sources;
mod steering;
mod step_progress;
mod story_points;
mod stuck_runs;
mod task_edits;
mod task_groups;
mod token_budgets;
mod tool_approvals;
mod unverified_steps;
mod worktree_not_created;
mod worktree_removals;

pub(super) use self::list::MIGRATIONS;

/// The SQL of `migration`, for a test that applies it again to a database it shaped as the schema
/// before it ([`super::apply_migration_for_test`]).
#[cfg(feature = "test-support")]
pub(super) const fn named(migration: super::ProjectMigration) -> &'static str {
  match migration {
    super::ProjectMigration::LocalIntegration => local_integration::MIGRATION,
    super::ProjectMigration::EditorDrafts => editor_drafts::MIGRATION,
  }
}
