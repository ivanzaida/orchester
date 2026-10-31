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
      "join-team",
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
      "slot-wait",
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
      "modules-none",
      "team-join",
      "team-join-wrong-code",
      "team-join-expired-code",
      "team-join-unreachable",
      "team-join-joined",
      "team-offer",
      "team-downloading",
      "team-download-failed",
      "team-download-progress",
      "team-download-stopped",
      "team-shared-projects",
      "team-shared-none"
    ]
  );
  for name in names {
    assert_eq!(scenarios::named(name).map(|scenario| scenario.name()), Some(name));
  }
  assert!(scenarios::named("Tasks").is_none());
}
