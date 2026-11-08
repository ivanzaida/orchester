//! A team task's Run on a pipeline this computer is not ready for (ORC-840, `Task · Run · not ready here`, R1
//! `Ku0qu`): the server is asked for nothing, and the Summary's card says *here*, one row for each part with its fix.
//! On the QA state `team-not-ready`, where KON-5 runs Team delivery.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_not_ready`

use std::path::Path;

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{HubFilter, ProjectPage, Route, TaskTab, TaskView},
  test_support::{Data, Window},
  ui::icon::Icon,
};
use orchester_core::{
  id::ProjectId,
  resource::ResourceKind,
  run::RunEventKind,
  secret::{SecretName, SecretValue},
  store::{ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::display;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::{
  start_failed::WITHIN,
  team::{self, State},
};

pub(super) fn task_route(project: ProjectId, key: &str) -> Route {
  let key = key.parse().expect("a task key");
  Route::Project(project, ProjectPage::task(key, TaskView::Summary, false))
}

#[path = "../../../workspace/tests/support/runner_programs.rs"]
mod runner_programs;

/// The window on the seed's `team-not-ready` (Kontur with the team's resources of `picker-readiness-team` and the
/// module branch of KON-5), joined to the stand-in team server with the real runner, which Run needs to be pressable;
/// on KON-5 once Run shows.
fn on_kon_5() -> (Window, TeamServer, ProjectId) {
  on_task(("team-not-ready", State::NotReadyHere), "KON-5")
}

/// The window on the seed's `scenario` joined to the stand-in team server in `state`, with the real runner; on the task
/// `key` once Run shows.
pub(super) fn on_task(setup: (&str, State), key: &str) -> (Window, TeamServer, ProjectId) {
  on_task_after_setup(setup, key, |_| {})
}

/// [`on_task`] with `before_open` run on the project's folder once the state is set up and before the window opens on
/// the task: what the project recorded before this window.
fn on_task_after_setup(
  (scenario, state): (&str, State),
  key: &str,
  before_open: impl FnOnce(&Path),
) -> (Window, TeamServer, ProjectId) {
  let dir = TempDir::new().expect("a temporary folder");
  let request = crate::kontur::SeedRequest {
    data_dir: dir.path().to_path_buf(),
    repo: dir.path().join("repo-kontur"),
    scenario: crate::kontur::scenarios::named(scenario).expect("a known scenario"),
    fake_codex: None,
    now: Timestamp::now(),
  };
  let seeded = crate::kontur::seed(&request).expect("the scenario seeds");
  let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
  let launch = data.launch_with(runner_programs::programs().runner.clone());
  let services = launch.services().expect("the session started");
  let server = team::prepare(state, services, data.runtime.handle()).expect("the state is set up");
  before_open(&data.repo("repo-kontur"));
  let mut window = Window::of(data, launch);
  window.navigation().go(task_route(seeded.project, key));
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

#[test]
fn team_qa_not_ready_header_line_names_the_computer_and_when_run_was_pressed() {
  let (mut window, _server, _) = on_kon_5();
  let before = Timestamp::now();
  refused(&mut window);
  let after = Timestamp::now();
  settle(&mut window);
  assert_eq!(
    window.text("task-status-label"),
    "Not started · not ready on this computer"
  );
  let pressed = window.text("task-run-line");
  let said = |at| format!("Run pressed {}", display::clock(at));
  assert!(
    pressed == said(before) || pressed == said(after),
    "the minute Run was pressed, with no earlier run to name: {pressed}"
  );
}

/// Takes KON-5 over from mara, which starts a new run that is refused as not ready here, until the card shows.
fn taken_over(window: &mut Window) {
  window.until("Take over", |window| window.texts("task-team-take-over").is_some());
  window.press("task-team-take-over");
  window.until("the question", |window| window.texts("task-take-over-title").is_some());
  window.press("task-take-over-confirm");
  window.until("the not-ready card", |window| window.texts("task-not-ready").is_some());
}

/// The KON-5 of `team-not-ready-ended` joined to the team server: the window on it.
fn on_kon_5_after_run() -> (Window, TeamServer, ProjectId) {
  on_task(("team-not-ready-ended", State::NotReadyAfterRun), "KON-5")
}

/// When KON-5's cancelled run ended, as the project's events record it.
fn cancelled_at(window: &Window) -> Timestamp {
  let db = ProjectDb::open(&window.data.repo("repo-kontur")).expect("the project");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == "KON-5")
    .expect("KON-5");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .remove(0)
    .expect("a readable run");
  db.events(run.id())
    .expect("events")
    .iter()
    .find(|event| matches!(event.kind(), RunEventKind::Cancelled))
    .map(|event| event.at())
    .expect("the run was cancelled")
}

/// The person gives the team's secret `GITHUB_TOKEN` a value on this computer.
fn give_secret_value(window: &Window) {
  let services = window.launch.services().expect("services");
  ProjectDb::open(&window.data.repo("repo-kontur"))
    .expect("the project")
    .set_team_secret_value(
      services.vault.get(),
      &SecretName::new("GITHUB_TOKEN").expect("a secret name"),
      &SecretValue::new("ghp_not-a-real-token-7d2e".to_owned()).expect("a value"),
    )
    .expect("the value is kept");
}

#[test]
fn team_qa_not_ready_header_line_says_when_the_earlier_run_ended() {
  let (mut window, _server, _) = on_kon_5_after_run();
  let before = Timestamp::now();
  taken_over(&mut window);
  let after = Timestamp::now();
  settle(&mut window);
  let ended = display::clock(cancelled_at(&window));
  let said = |at| format!("Run pressed {} · run 8 ended {ended}", display::clock(at));
  let line = window.text("task-run-line");
  assert!(line == said(before) || line == said(after), "{line}");
  assert_eq!(
    window.text("task-status-label"),
    "Not started · not ready on this computer"
  );
}

/// A refusal the project recorded before this window opened (a start that failed for good as not ready here) shows the
/// card, and the line says nothing of a press this window never saw.
#[test]
fn team_qa_not_ready_header_line_of_a_refusal_recorded_before_the_window_leaves_the_press_out() {
  let failure = r#"{"failure":{"kind":"not_ready_here","reason":{"pipeline":"Team delivery","missing":[{"missing":{"kind":"secret","name":"GITHUB_TOKEN"},"name":"GITHUB_TOKEN"}]}},"claim":"released"}"#;
  let (mut window, _server, _) =
    on_task_after_setup(("team-start-past-failure", State::StartPastFailure), "KON-5", |root| {
      rusqlite::Connection::open(root.join(".orchester/orchester.db"))
        .expect("open")
        .execute_batch(&format!(
          "DROP TRIGGER team_starts_keep_request; UPDATE team_starts SET failure = '{failure}'"
        ))
        .expect("the recorded start failed as not ready here");
    });
  window.until_within(WITHIN, "the card or a notice", |window| {
    window.texts("task-not-ready").is_some() || window.texts("task-team-refused").is_some()
  });
  assert!(
    window.texts("task-not-ready").is_some(),
    "{:?}",
    window.texts("task-team-refused")
  );
  settle(&mut window);
  assert_eq!(
    window.text("task-status-label"),
    "Not started · not ready on this computer"
  );
  assert!(
    window.texts("task-run-line").is_none(),
    "no press time is known: {:?}",
    window.texts("task-run-line")
  );
}

#[test]
fn team_qa_not_ready_header_line_goes_back_once_the_card_finds_nothing_missing_on_each_tab() {
  let (mut window, _server, project) = on_kon_5_after_run();
  taken_over(&mut window);
  settle(&mut window);
  assert_eq!(
    rows(&mut window),
    ["pencil-design", "Secret GITHUB_TOKEN"],
    "the program and the secret are all this computer lacks"
  );
  assert_eq!(
    window.text("task-status-label"),
    "Not started · not ready on this computer"
  );

  // The person fixes both and comes back to the task, as the card's fixes lead them to.
  set_pencil_path(&window);
  give_secret_value(&window);
  let key = "KON-5".parse().expect("a task key");
  let page = |view| Route::Project(project, ProjectPage::task(Clone::clone(&key), view, false));
  window.navigation().go(Route::Project(project, ProjectPage::Agents));
  window.pass();
  window.navigation().go(page(TaskView::of(TaskTab::Conversation)));
  window.until("the notice goes", |window| {
    window.texts("task-not-ready-notice").is_none()
  });
  settle(&mut window);
  assert_normal_line(&mut window);

  window.navigation().go(page(TaskView::Summary));
  window.until("the card goes", |window| window.texts("task-not-ready").is_none());
  settle(&mut window);
  assert_normal_line(&mut window);
}

/// The ordinary line of KON-5, whose latest run is the cancelled one.
fn assert_normal_line(window: &mut Window) {
  assert_eq!(window.text("task-status-label"), "Canceled");
  assert!(
    window.text("task-run-line").starts_with("Run 8"),
    "{}",
    window.text("task-run-line")
  );
}
