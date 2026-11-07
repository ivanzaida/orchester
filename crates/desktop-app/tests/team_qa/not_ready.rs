//! A team task's Run on a pipeline this computer is not ready for (ORC-840, `Task · Run · not ready here`, R1
//! `Ku0qu`): the server is asked for nothing, and the Summary's card says *here*, one row for each part with its fix.
//! On the QA state `team-not-ready`, where KON-5 runs Team delivery.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_not_ready`

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, ProjectPage, Route, TaskTab, TaskView},
  test_support::{Data, Window},
  ui::icon::Icon,
};
use orchester_core::{id::ProjectId, resource::ResourceKind, store::ResourceStore, time::Timestamp};
use orchester_store::{AppDb, ProjectDb};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::team::{self, State};

fn task_route(project: ProjectId, key: &str) -> Route {
  let key = key.parse().expect("a task key");
  Route::Project(project, ProjectPage::task(key, TaskView::Summary, false))
}

#[path = "../../../workspace/tests/support/runner_programs.rs"]
mod runner_programs;

/// The window on the seed's `team-not-ready` (Kontur with the team's resources of `picker-readiness-team` and the
/// module branch of KON-5), joined to the stand-in team server with the real runner, which Run needs to be pressable;
/// on KON-5 once Run shows.
fn on_kon_5() -> (Window, TeamServer, ProjectId) {
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
  window.navigation().go(task_route(seeded.project, "KON-5"));
  window.until("Run", |window| window.texts("task-run").is_some());
  (window, server, seeded.project)
}

#[test]
fn team_qa_not_ready_run_says_here_and_lists_the_parts_with_a_hub_fix() {
  let (mut window, server, _) = on_kon_5();
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

/// Lets the card's own re-read finish: it asks again as it shows, and what it finds replaces what the start refused.
fn settle(window: &mut Window) {
  for _ in 0..60 {
    window.tree.tick_futures();
    window.pass();
    std::thread::sleep(std::time::Duration::from_millis(5));
  }
}

/// The ids of the rows the card lists.
fn rows(window: &mut Window) -> Vec<String> {
  (0..12)
    .filter_map(|index| {
      window
        .texts(&format!("task-not-ready-row-{index}-what"))
        .map(|what| what.join(" "))
    })
    .collect()
}

/// Runs KON-5 until the card shows.
fn refused(window: &mut Window) {
  window.until("the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  window.press("task-run");
  window.until("the not-ready card", |window| window.texts("task-not-ready").is_some());
}

/// The person sets the program path of the team's `pencil-design` on this computer, as the card's fix does.
fn set_pencil_path(window: &Window) {
  let root = window.data.repo("repo-kontur");
  let db = ProjectDb::open(&root).expect("the project");
  let server = db.sharing().expect("read").expect("a team project").server;
  let pencil = db
    .resources(ResourceKind::McpServer)
    .expect("servers")
    .into_iter()
    .find(|resource| resource.name().as_str() == "pencil-design")
    .expect("the team's pencil-design");
  let program = std::env::current_exe().expect("a program that exists");
  AppDb::open(&window.data.database())
    .expect("the application database")
    .set_mcp_server_program(server, pencil.id(), &program, Timestamp::now())
    .expect("the path is kept");
}

#[test]
fn team_qa_not_ready_card_follows_the_team_start_until_the_team_parts_are_fixed() {
  let (mut window, _server, project) = on_kon_5();
  refused(&mut window);
  let before = rows(&mut window);
  // The card asked again as it showed; a team task is no task of this computer's queue, so asking as a local start
  // would find nothing missing and clear the card. It is read after the re-read settled, not before.
  settle(&mut window);
  assert_eq!(rows(&mut window), before, "the re-read finds what the start found");
  assert_eq!(
    before,
    ["Builder", "Reviewer", "pencil-design", "Secret GITHUB_TOKEN"],
    "a connection for each profile, the program and the secret"
  );

  // On another tab the notice reads as the card does.
  let key = "KON-5".parse().expect("a task key");
  let page = |view| Route::Project(project, ProjectPage::task(Clone::clone(&key), view, false));
  window.navigation().go(page(TaskView::of(TaskTab::Conversation)));
  window.until("the notice", |window| window.texts("task-not-ready-notice").is_some());
  settle(&mut window);
  assert!(window.texts("task-not-ready-notice").is_some(), "still not ready here");

  // The person sets the program path and comes back: what the card lists is what is still missing.
  set_pencil_path(&window);
  window.navigation().go(page(TaskView::Summary));
  window.until("the card without the program", |window| {
    rows(window) == ["Builder", "Reviewer", "Secret GITHUB_TOKEN"]
  });
  settle(&mut window);
  assert_eq!(rows(&mut window), ["Builder", "Reviewer", "Secret GITHUB_TOKEN"]);
}
