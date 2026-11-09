//! The stopped run's card in a local project (`Run state · Stopped · archived` `pLdtd`, ORC-848 D3): the person
//! archived it, not a team, so the card, the status line and the last event say *you* and never *the team*. On the
//! seed's `local-hub-archived`, where KON-11 was stopped by the archive of the profile Reviewer and KON-9 by that of
//! the MCP server postgres-dev.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_local_archived`

use orchester_app::test_support::{Data, Window};
use orchester_core::time::Timestamp;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::not_ready::task_route;

/// The window on the seed's `local-hub-archived`, on the task `key` once its archive card shows.
fn on_task(key: &str) -> Window {
  let dir = TempDir::new().expect("a temporary folder");
  let request = crate::kontur::SeedRequest {
    data_dir: dir.path().to_path_buf(),
    repo: dir.path().join("repo-kontur"),
    scenario: crate::kontur::scenarios::named("local-hub-archived").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = crate::kontur::seed(&request).expect("the scenario seeds");
  let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
  let launch = data.launch();
  let mut window = Window::of(data, launch);
  window.navigation().go(task_route(seeded.project, key));
  window.until("the card", |window| window.texts("task-archived").is_some());
  window
}

#[test]
fn team_qa_local_archived_a_profile_the_person_archived_is_not_the_teams() {
  let mut window = on_task("KON-11");
  let body = window.text("task-archived-body");
  assert!(
    body.starts_with("You archived the agent profile Reviewer at "),
    "{body}"
  );
  let need = window.text("task-archived-resource-need");
  assert!(need.starts_with("agent profile · archived by you · "), "{need}");
  window.until("the last event", |window| window.texts("task-event-text").is_some());
  let event = window.text("task-event-text");
  assert!(event.contains("Reviewer was archived, which this run used."), "{event}");
  for words in [&body, &need, &event] {
    assert!(
      !words.to_lowercase().contains("team"),
      "a local project has no team: {words}"
    );
  }
}

#[test]
fn team_qa_local_archived_an_mcp_server_the_person_archived_names_no_team_either() {
  let mut window = on_task("KON-9");
  let body = window.text("task-archived-body");
  assert!(body.starts_with("postgres-dev was archived at "), "{body}");
  assert!(!body.to_lowercase().contains("team"), "{body}");
}
