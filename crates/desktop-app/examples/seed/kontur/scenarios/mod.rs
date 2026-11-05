//! The scenarios the seed writes, one file each, and their registry ([`ALL`]): one line per scenario.

mod accounts;
mod agents;
mod agents_browser;
mod agents_cases;
mod agents_library;
mod api_providers;
mod approval_answers;
mod approval_modes;
mod auto_mode;
mod browser;
mod browser_window;
mod cards;
mod changes;
mod connections;
mod conversation;
mod decision_connections;
mod decision_reviews;
mod decisions;
mod deliverables;
mod delivery;
mod empty;
mod import;
mod interrupted;
mod lifecycle;
mod live;
mod loading_states;
mod long_chats;
mod memory;
mod merge_attempts;
mod merges;
mod modules;
mod native;
mod orchestrator;
mod orchestrator_plan;
mod orchestrator_rewind;
mod overview_states;
mod ownership;
mod pipelines;
mod project_settings;
mod pull_requests;
pub mod qa_app_asks;
mod review_changed;
mod review_page;
mod reviewer_rows;
mod run_context;
mod run_model;
pub mod settings;
mod shared_config;
mod shared_here;
mod side_question;
mod slot_wait;
mod sources;
mod storage;
mod task_merge_card;
mod task_merge_row;
mod task_states;
mod tasks;
mod tasks_memory;
mod tasks_unreadable;
pub mod team;
mod todo_lists;
mod unreadable_db;
mod usage;
mod usage_accounts;
mod usage_providers;

use super::{Kontur, SeedError};

type Seed = fn(&mut Kontur) -> Result<(), SeedError>;

/// One scenario: its name on the command line and what it writes.
#[derive(Clone, Copy)]
pub struct Scenario {
  name: &'static str,
  seed: Seed,
  /// Its runs are live runs of `orchester-fake-codex`, which it cannot do without.
  needs_fake_codex: bool,
}

impl Scenario {
  const fn of(name: &'static str, seed: Seed) -> Self {
    Self {
      name,
      seed,
      needs_fake_codex: false,
    }
  }

  const fn with_fake_codex(name: &'static str, seed: Seed) -> Self {
    Self {
      name,
      seed,
      needs_fake_codex: true,
    }
  }

  pub const fn name(self) -> &'static str {
    self.name
  }

  pub const fn needs_fake_codex(self) -> bool {
    self.needs_fake_codex
  }

  pub(super) fn seed(self, kontur: &mut Kontur) -> Result<(), SeedError> {
    (self.seed)(kontur)
  }
}

pub const ALL: [Scenario; 159] = [
  Scenario::of("reviewer-row", reviewer_rows::seed),
  Scenario::of("reviewer-row-local", reviewer_rows::local),
  Scenario::of("reviewer-row-unavailable", reviewer_rows::unavailable),
  Scenario::of("agents", agents::seed),
  Scenario::of("agents-library", agents_library::seed),
  Scenario::of("agents-browser", agents_browser::seed),
  Scenario::of("browser-window", browser_window::default),
  Scenario::of("browser-window-many", browser_window::many),
  Scenario::of("browser-window-overflow", browser_window::overflow),
  Scenario::of("browser-window-acting", browser_window::acting),
  Scenario::of("browser-window-waiting", browser_window::waiting),
  Scenario::of("browser-window-dialog", browser_window::dialog),
  Scenario::of("browser-window-file-chooser", browser_window::file_chooser),
  Scenario::of("browser-window-file-chosen", browser_window::file_chosen),
  Scenario::of("browser-window-select", browser_window::select),
  Scenario::of("browser-window-headed", browser_window::headed),
  Scenario::of("browser-window-empty", browser_window::empty),
  Scenario::of("browser-window-not-found", browser_window::not_found),
  Scenario::of("agents-empty", agents::empty),
  Scenario::of("agents-no-connections", agents::no_connections),
  Scenario::of("agents-unreadable", agents::unreadable),
  Scenario::of("empty", empty::seed),
  Scenario::of("join-team", empty::seed),
  Scenario::of("tasks", tasks::seed),
  Scenario::of("tasks-memory", tasks_memory::seed),
  Scenario::of("memory-only", tasks_memory::alone),
  Scenario::of("memory", memory::seed),
  Scenario::of("task-states", task_states::seed),
  Scenario::of("conversation", conversation::seed),
  Scenario::of("conversation-context-plenty", run_context::plenty),
  Scenario::of("conversation-context-near-full", run_context::near_full),
  Scenario::of("conversation-context-full", run_context::full),
  Scenario::of("conversation-model", run_model::seed),
  Scenario::of("conversation-model-changed", run_model::changed),
  Scenario::of("conversation-model-waits", run_model::waits),
  Scenario::of("conversation-model-fixed", run_model::fixed),
  Scenario::of("conversation-model-back", run_model::back),
  Scenario::of("side-question", side_question::seed),
  Scenario::of("side-question-sent", side_question::sent),
  Scenario::of("long-chats", long_chats::seed),
  Scenario::of("orchestrator", orchestrator::seed),
  Scenario::of("orchestrator-not-set-up", orchestrator::not_set_up),
  Scenario::of("orchestrator-on-codex", orchestrator::on_codex),
  Scenario::of("orchestrator-codex-limit", orchestrator::codex_limit),
  Scenario::of("orchestrator-context-plenty", orchestrator::context::plenty),
  Scenario::of("orchestrator-context-near-full", orchestrator::context::near_full),
  Scenario::of("orchestrator-context-full", orchestrator::context::full),
  Scenario::of("orchestrator-context-unknown", orchestrator::context::unknown),
  Scenario::of("orchestrator-compacting", orchestrator::context::compacting),
  Scenario::of("orchestrator-compacted", orchestrator::context::compacted),
  Scenario::of("orchestrator-compact-failed", orchestrator::context::compact_failed),
  Scenario::of("orchestrator-plan-only", orchestrator_plan::plan_only),
  Scenario::of("orchestrator-plan-answer", orchestrator_plan::answer),
  Scenario::of("orchestrator-plan-approved", orchestrator_plan::approved),
  Scenario::of("orchestrator-plan-changed", orchestrator_plan::changed),
  Scenario::of("orchestrator-rewind", orchestrator_rewind::seed),
  Scenario::of("orchestrator-rewound", orchestrator_rewind::rewound),
  Scenario::of("orchestrator-forked", orchestrator_rewind::forked),
  Scenario::of("changes", changes::seed),
  Scenario::of("cards", cards::seed),
  Scenario::of("cards-auto-mode", auto_mode::seed),
  Scenario::of("cards-approval-modes", approval_answers::seed),
  Scenario::of("approval-modes", approval_modes::seed),
  Scenario::of("approval-modes-project", approval_modes::project),
  Scenario::of("decisions", decisions::seed),
  Scenario::of("deliverables", deliverables::seed),
  Scenario::of("delivery", delivery::seed),
  Scenario::of("delivery-blocked", delivery::blocked),
  Scenario::of("delivery-lanes", delivery::lanes),
  Scenario::of("delivery-empty", delivery::empty),
  Scenario::of("merges", merges::seed),
  Scenario::of("merges-outcomes", merges::outcomes),
  Scenario::of("merges-empty", merges::empty),
  Scenario::of("merges-many", merges::many),
  Scenario::of("merge-attempts", merge_attempts::seed),
  Scenario::of("review-page", review_page::seed),
  Scenario::of("review-changed", review_changed::seed),
  Scenario::of("task-merge-row", task_merge_row::seed),
  Scenario::of("task-merge-card-ready", task_merge_card::ready_to_merge),
  Scenario::of("task-merge-card-refused", task_merge_card::refused_at_lock),
  Scenario::of("task-merge-card-conflicts", task_merge_card::conflicts),
  Scenario::of("overview-quiet", delivery::quiet),
  Scenario::of("overview-states", overview_states::seed),
  Scenario::of("interrupted", interrupted::seed),
  Scenario::of("unreadable-db", unreadable_db::seed),
  Scenario::of("tasks-unreadable", tasks_unreadable::seed),
  Scenario::with_fake_codex("live", live::seed),
  Scenario::with_fake_codex("lifecycle", lifecycle::seed),
  Scenario::with_fake_codex("slot-wait", slot_wait::seed),
  Scenario::of("connections", connections::seed),
  Scenario::of("decision-connections", decision_connections::seed),
  Scenario::of("decision-connections-missing", decision_connections::missing),
  Scenario::of("api-providers", api_providers::seed),
  Scenario::of("native", native::seed),
  Scenario::of("project-settings", project_settings::seed),
  Scenario::of("project-settings-shared", project_settings::shared),
  Scenario::of("shared-config", shared_config::admin),
  Scenario::of("shared-config-member", shared_config::member),
  Scenario::of("shared-config-here", shared_here::seed),
  Scenario::of("shared-config-chosen", shared_here::chosen),
  Scenario::of("shared-config-no-claude", shared_here::no_claude),
  Scenario::of("pipelines", pipelines::seed),
  Scenario::of("pipelines-empty", pipelines::empty),
  Scenario::of("pipelines-publish", pipelines::publish),
  Scenario::of("accounts", accounts::seed),
  Scenario::of("settings", settings::seed),
  Scenario::of("browser", browser::seed),
  Scenario::of("browser-not-found", browser::not_found),
  Scenario::of("browser-chosen", browser::chosen),
  Scenario::of("pull-requests", pull_requests::seed),
  Scenario::of("pull-request-feedback", pull_requests::feedback::seed),
  Scenario::of("sources", sources::seed),
  Scenario::of("sources-unreadable", sources::unreadable),
  Scenario::of("usage", usage::seed),
  Scenario::of("usage-empty", usage::no_runs_today),
  Scenario::of("usage-bare", usage::bare),
  Scenario::of("import", import::seed),
  Scenario::of("import-done", import::done),
  Scenario::of("modules", modules::seed),
  Scenario::of("modules-short", modules::short),
  Scenario::of("modules-none", modules::none),
  Scenario::of("ownership", ownership::seed),
  Scenario::of("ownership-signed-in", ownership::signed_in),
  Scenario::of("ownership-join-review", ownership::join_review),
  Scenario::of("ownership-ended", ownership::ended),
  Scenario::of("ownership-nothing-here", ownership::nothing_here),
  Scenario::of("invite-signed-out", ownership::invite),
  Scenario::of("invite-sign-in-again", ownership::invite_sign_in_again),
  Scenario::of("team-join", team::join),
  Scenario::of("team-join-wrong-code", team::wrong_code),
  Scenario::of("team-join-expired-code", team::expired_code),
  Scenario::of("team-join-unreachable", team::unreachable),
  Scenario::of("team-join-joined", team::joined),
  Scenario::of("team-offer", team::offer),
  Scenario::of("team-downloading", team::downloading),
  Scenario::of("team-download-failed", team::download_failed),
  Scenario::of("team-download-progress", team::download_progress),
  Scenario::of("team-download-stopped", team::download_stopped),
  Scenario::of("team-shared-projects", team::shared_projects),
  Scenario::of("team-shared-none", team::shared_none),
  Scenario::of("team-clone", team::clones),
  Scenario::of("team-member", team::member),
  Scenario::of("team-admin", team::admin),
  Scenario::of("team-server-admin", team::server_admin),
  Scenario::of("team-outage", team::outage),
  Scenario::of("team-outage-running", team::outage_running),
  Scenario::of("team-outage-later", team::outage_later),
  Scenario::of("team-member-running", team::member_running),
  Scenario::of("team-module-outage", team::module_outage),
  Scenario::of("team-claims", team::claims),
  Scenario::of("team-module-work", team::module_work),
  Scenario::of("team-admin-work", team::admin_work),
  Scenario::of("team-version-refused", team::version_refused),
  Scenario::with_fake_codex("team-running", team::team_running),
  Scenario::of("team-edit-conflict", team::edit_conflict),
  Scenario::of("modules-base-branch", modules::base_branch),
  Scenario::of("loading-states", loading_states::seed),
  Scenario::of("storage-low-disk", storage::low_disk),
  Scenario::of("storage-empty", storage::nothing),
];

/// The scenario called `name`.
pub fn named(name: &str) -> Option<Scenario> {
  ALL.into_iter().find(|scenario| scenario.name == name)
}
