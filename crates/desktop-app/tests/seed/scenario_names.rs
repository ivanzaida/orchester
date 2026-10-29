//! The seed's scenario names: each listed once, in order, and each found by its own name.

use crate::kontur::scenarios;

#[test]
fn seed_names_every_scenario_once() {
  let names: Vec<&str> = scenarios::ALL.iter().map(|scenario| scenario.name()).collect();
  assert_eq!(
    names,
    [
      "agents",
      "agents-library",
      "agents-empty",
      "agents-no-connections",
      "agents-unreadable",
      "empty",
      "tasks",
      "tasks-memory",
      "memory-only",
      "memory",
      "task-states",
      "conversation",
      "long-chats",
      "orchestrator",
      "orchestrator-not-set-up",
      "changes",
      "cards",
      "cards-auto-mode",
      "cards-approval-modes",
      "approval-modes",
      "approval-modes-project",
      "decisions",
      "deliverables",
      "delivery",
      "delivery-blocked",
      "delivery-empty",
      "overview-quiet",
      "overview-states",
      "interrupted",
      "unreadable-db",
      "tasks-unreadable",
      "live",
      "lifecycle",
      "connections",
      "api-providers",
      "native",
      "project-settings",
      "project-settings-shared",
      "pipelines",
      "pipelines-empty",
      "pipelines-publish",
      "accounts",
      "settings",
      "pull-requests",
      "pull-request-feedback",
      "sources",
      "sources-unreadable",
      "usage",
      "usage-empty",
      "usage-bare",
      "import",
      "import-done",
      "modules",
      "modules-short",
      "modules-none"
    ]
  );
  for name in names {
    assert_eq!(scenarios::named(name).map(|scenario| scenario.name()), Some(name));
  }
  assert!(scenarios::named("Tasks").is_none());
}
