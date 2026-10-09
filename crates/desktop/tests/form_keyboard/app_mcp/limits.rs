//! A project's limits through Orchester's MCP server, with the real runner: `list_projects` shows
//! them, `update_project_limits` changes the ones it names with the Limits tab's checks, and the
//! runner takes a changed agent count at once.

use orchester_core::store::SettingsStore;
use orchester_store::ProjectDb;
use serde_json::json;

use super::{agent, ok, refused};
use crate::{
  github::shows,
  harness::Desktop,
  task_run::{init_repository, programs, seed},
};

fn status_agents(desktop: &mut Desktop, text: &str) -> bool {
  desktop.tree.get_element_by_id("status-agents").is_some() && shows(desktop, "status-agents", text)
}

#[test]
fn app_mcp_reads_and_changes_the_project_limits() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| seed(repo, &fake, false));
  let repo = desktop.repo().to_path_buf();
  let mut agent = agent(&desktop);
  desktop.wait_until("the runner reports its slots", |desktop| status_agents(desktop, "0/4"));
  let listed = ok(&mut agent, "list_projects", &json!({}));
  assert_eq!(
    listed["projects"][0]["limits"],
    json!({
      "default_run_budget": null,
      "daily_budget": 20_000_000,
      "default_time_limit_minutes": null,
      "agent_slots": 4,
    })
  );

  let changed = ok(
    &mut agent,
    "update_project_limits",
    &json!({"project": "Kontur", "agent_slots": 2, "daily_budget": 30_000_000, "default_time_limit_minutes": 45}),
  );
  assert_eq!(
    changed["limits"],
    json!({
      "default_run_budget": null,
      "daily_budget": 30_000_000,
      "default_time_limit_minutes": 45,
      "agent_slots": 2,
    })
  );
  let stored = || ProjectDb::open(&repo).expect("open").project_limits().expect("limits");
  assert_eq!(stored().agent_slots().get().get(), 2);
  assert_eq!(stored().daily_budget().get(), 30_000_000);
  desktop.wait_until("the runner uses two slots", |desktop| status_agents(desktop, "0/2"));
  let listed = ok(&mut agent, "list_projects", &json!({}));
  assert_eq!(listed["projects"][0]["limits"]["agent_slots"], 2);

  // Each invalid limit is refused in the Limits tab's words, and nothing is saved.
  let saved = stored();
  let error = refused(
    &mut agent,
    "update_project_limits",
    &json!({"project": "Kontur", "agent_slots": 17, "default_run_budget": 12_000_000}),
  );
  assert_eq!(error["code"], "invalid_limits");
  assert_eq!(error["reasons"], json!(["Enter a whole number from 1 to 16."]));
  let error = refused(
    &mut agent,
    "update_project_limits",
    &json!({"project": "Kontur", "default_run_budget": 12_000_000, "default_time_limit_minutes": 0}),
  );
  assert_eq!(error["reasons"], json!(["The time limit must be at least one minute."]));
  let error = refused(
    &mut agent,
    "update_project_limits",
    &json!({"project": "Kontur", "agent_slots": "four"}),
  );
  assert_eq!(error["code"], "invalid_arguments");
  let error = refused(
    &mut agent,
    "update_project_limits",
    &json!({"project": "Kontur", "max_run_budget": 10_000_000}),
  );
  assert_eq!(error["code"], "invalid_arguments", "there is no maximum run budget");
  assert_eq!(stored(), saved);

  // No maximum, and `null` is no limit.
  let changed = ok(
    &mut agent,
    "update_project_limits",
    &json!({"project": "Kontur", "default_run_budget": 900_000_000}),
  );
  assert_eq!(changed["limits"]["default_run_budget"], 900_000_000);
  let changed = ok(
    &mut agent,
    "update_project_limits",
    &json!({"project": "Kontur", "default_run_budget": null, "default_time_limit_minutes": null}),
  );
  assert_eq!(changed["limits"]["default_run_budget"], json!(null));
  assert_eq!(changed["limits"]["default_time_limit_minutes"], json!(null));
  assert_eq!(stored().default_time_limit(), None);
}
