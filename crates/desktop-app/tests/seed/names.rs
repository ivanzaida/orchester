//! The seed's registry of scenarios: each name once, in the registry's order, found by its exact name only.

use super::kontur::scenarios;

#[test]
fn seed_names_every_scenario_once() {
  let names: Vec<&str> = scenarios::ALL.iter().map(|scenario| scenario.name()).collect();
  assert_eq!(
    names,
    [
      "agents",
      "agents-library",
      "agents-browser",
      "browser-window",
      "browser-window-many",
      "browser-window-overflow",
      "browser-window-acting",
      "browser-window-waiting",
      "browser-window-dialog",
      "browser-window-select",
      "browser-window-headed",
      "browser-window-empty",
      "browser-window-not-found",
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
      "browser",
      "browser-not-found",
      "browser-chosen",
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
