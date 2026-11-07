//! A team task's Run on a pipeline this computer is not ready for (ORC-840, `Task · Run · not ready here`, R1
//! `Ku0qu`): the server is asked for nothing, and the Summary's card says *here*, one row for each part with its fix.
//! On the QA state `team-not-ready`, where KON-5 runs Team delivery.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_not_ready`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, ProjectPage, Route, TaskView},
  test_support::{Data, Window},
  ui::icon::Icon,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::{
  team::{self, State},
  team_work_actions::own_repository,
};

fn task_route(project: ProjectId, key: &str) -> Route {
  let key = key.parse().expect("a task key");
  Route::Project(project, ProjectPage::task(key, TaskView::Summary, false))
}

#[path = "../../../workspace/tests/support/runner_programs.rs"]
mod runner_programs;

/// The window on the seed's `team-not-ready` (Kontur with the team's resources of `picker-readiness-team`), joined to
/// the stand-in team server with the real runner, which Run needs to be pressable; on KON-5 once Run shows.
fn on_kon_5() -> (Window, TeamServer) {
  let dir = TempDir::new().expect("a temporary folder");
  let request = crate::kontur::SeedRequest {
    data_dir: dir.path().to_path_buf(),
    repo: dir.path().join("repo-kontur"),
    scenario: crate::kontur::scenarios::named("team-not-ready").expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = crate::kontur::seed(&request).expect("the scenario seeds");
  let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
  let launch = data.launch_with(runner_programs::programs().runner.clone());
  let services = launch.services().expect("the session started");
  let server = team::prepare(State::NotReadyHere, services, data.runtime.handle()).expect("the state is set up");
  let mut window = Window::of(data, launch);
  own_repository(&window.data.repo("repo-kontur"));
  window.navigation().go(task_route(seeded.project, "KON-5"));
  window.until("Run", |window| window.texts("task-run").is_some());
  (window, server)
}

#[test]
fn team_qa_not_ready_run_says_here_and_lists_the_parts_with_a_hub_fix() {
  let (mut window, server) = on_kon_5();
  window.until("the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  window.press("task-run");
  window.until("the not-ready card", |window| {
    window.texts("task-not-ready").is_some() || window.texts("task-team-refused").is_some()
  });
  assert!(
    window.texts("task-not-ready").is_some(),
    "{:?}",
    window.texts("task-team-refused")
  );

  assert_eq!(
    window.text("task-not-ready-title"),
    "Not ready on this computer · nothing started"
  );
  assert!(
    window.text("task-not-ready-meta").starts_with("Team delivery"),
    "{}",
    window.text("task-not-ready-meta")
  );
  assert!(window.text("task-not-ready-body").contains("They stay here"));
  assert_eq!(
    window.text("task-not-ready-open-agents"),
    "Open Agents · Needs you here"
  );
  assert_eq!(
    server
      .commands()
      .iter()
      .filter(|asked| *asked == "claim.acquire")
      .count(),
    0,
    "the server was asked for nothing"
  );

  let set_path = (0..8)
    .map(|index| format!("task-not-ready-row-{index}-fix"))
    .find(|id| window.texts(id).is_some_and(|texts| texts.concat() == "Set path →"))
    .expect("a row to set the program path");
  window.press(&set_path);
  // A team MCP server's program path is set from the hub's MCP server panel (ORC-833); until it is built the fix opens
  // the server in the hub.
  let Route::Project(_, ProjectPage::AgentsHub(view)) = window.navigation().route_now() else {
    panic!("the fix opens a resource of the Agents hub");
  };
  assert_eq!(view.filter, HubFilter::McpServers);
  assert!(view.item.is_some(), "the server itself");
}
