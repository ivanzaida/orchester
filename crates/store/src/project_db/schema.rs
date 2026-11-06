//! Project database migrations. Append only: never edit a migration that has shipped. Later
//! migrations live in modules of their own, named after what they add.

mod agent_accounts;
mod agent_files;
mod agent_slots;
mod api_providers;
mod approval_choices;
mod approval_modes;
mod approve_everything;
mod archive_stops;
mod attachments;
mod auto_mode;
mod automation_answers;
mod automation_drafts;
mod base_branch;
mod base_notice_merges;
mod base_notices;
mod base_sources;
mod ci_fixes;
mod code_index;
mod command_approvals;
mod context_use;
mod conversation;
mod creation_snapshots;
mod decision_authors;
mod decision_connections;
mod decision_review;
mod decision_reviewer;
mod decision_tests;
mod default_pipeline;
mod denial_notes;
mod disk_guard;
mod editor_drafts;
mod feedback;
mod flaky_tests;
mod import;
mod invocation_permissions;
mod joining;
mod list;
mod local_integration;
mod mcp;
mod memory;
mod merge_attempts;
mod merge_queue;
mod modules;
mod native_commands;
mod native_conversations;
mod optional_daily_budget;
mod optional_limits;
mod orchestrator;
mod orchestrator_accounts;
mod orchestrator_archive;
mod orchestrator_queue;
mod orchestrator_reports;
mod orchestrator_run_ends;
mod orchestrator_senders;
mod orchestrator_sent;
mod orchestrator_switches;
mod orchestrator_titles;
mod orchestrator_todos;
mod orchestrator_waits;
mod organization;
mod permission_events;
mod permitted_decisions;
mod plan_decided_by;
mod plan_limits;
mod plan_only;
mod project_approval_rules;
mod pull_request_syncs;
mod qa_slots;
mod questions;
mod queue_holds;
mod recovery;
mod removed_resources;
mod reported_windows;
mod request_answers;
mod returned_after_end;
mod review_threads;
mod reviewer_accounts;
mod reviewer_usage;
mod run_completions;
mod run_inputs;
mod run_models;
mod run_rebases;
mod run_resolutions;
mod runner_failures;
mod service_slots;
mod services;
mod shared_configuration;
mod shared_members;
mod shared_publishes;
mod sharing;
mod side_questions;
mod sources;
mod steering;
mod step_progress;
mod story_points;
mod stuck_runs;
mod task_edits;
mod task_groups;
mod task_identities;
mod team_delivery;
mod team_delivery_waits;
mod team_operations;
mod team_reviewer;
mod team_work;
mod test_suites;
mod token_budgets;
mod tool_approvals;
mod unverified_steps;
mod usage_providers;
mod waiting_versions;
mod workspaces;
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
