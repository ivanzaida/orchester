//! The scenarios the seed writes, one file each, and their registry ([`ALL`]): one line per scenario.

mod accounts;
mod agents;
mod agents_cases;
mod agents_library;
mod api_providers;
mod approval_answers;
mod approval_modes;
mod auto_mode;
mod cards;
mod changes;
mod connections;
mod conversation;
mod decisions;
mod deliverables;
mod delivery;
mod empty;
mod import;
mod interrupted;
mod lifecycle;
mod live;
mod long_chats;
mod memory;
mod modules;
mod native;
mod orchestrator;
mod overview_states;
mod pipelines;
mod project_settings;
mod pull_requests;
pub mod settings;
mod sources;
mod task_states;
mod tasks;
mod tasks_memory;
mod tasks_unreadable;
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

pub const ALL: [Scenario; 55] = [
  Scenario::of("agents", agents::seed),
  Scenario::of("agents-library", agents_library::seed),
  Scenario::of("agents-empty", agents::empty),
  Scenario::of("agents-no-connections", agents::no_connections),
  Scenario::of("agents-unreadable", agents::unreadable),
  Scenario::of("empty", empty::seed),
  Scenario::of("tasks", tasks::seed),
  Scenario::of("tasks-memory", tasks_memory::seed),
  Scenario::of("memory-only", tasks_memory::alone),
  Scenario::of("memory", memory::seed),
  Scenario::of("task-states", task_states::seed),
  Scenario::of("conversation", conversation::seed),
  Scenario::of("long-chats", long_chats::seed),
  Scenario::of("orchestrator", orchestrator::seed),
  Scenario::of("orchestrator-not-set-up", orchestrator::not_set_up),
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
  Scenario::of("delivery-empty", delivery::empty),
  Scenario::of("overview-quiet", delivery::quiet),
  Scenario::of("overview-states", overview_states::seed),
  Scenario::of("interrupted", interrupted::seed),
  Scenario::of("unreadable-db", unreadable_db::seed),
  Scenario::of("tasks-unreadable", tasks_unreadable::seed),
  Scenario::with_fake_codex("live", live::seed),
  Scenario::with_fake_codex("lifecycle", lifecycle::seed),
  Scenario::of("connections", connections::seed),
  Scenario::of("api-providers", api_providers::seed),
  Scenario::of("native", native::seed),
  Scenario::of("project-settings", project_settings::seed),
  Scenario::of("project-settings-shared", project_settings::shared),
  Scenario::of("pipelines", pipelines::seed),
  Scenario::of("pipelines-empty", pipelines::empty),
  Scenario::of("pipelines-publish", pipelines::publish),
  Scenario::of("accounts", accounts::seed),
  Scenario::of("settings", settings::seed),
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
];

/// The scenario called `name`.
pub fn named(name: &str) -> Option<Scenario> {
  ALL.into_iter().find(|scenario| scenario.name == name)
}
