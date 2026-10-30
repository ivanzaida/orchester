//! The QA app's team scenarios (`examples/qa_app/team`), each state on the headless window with the stand-in team
//! server: what the person's presses reach on *Connect a server*, and what the session's own work reaches with Kontur
//! linked and joined. Nothing here starts `tools/qa-app.ps1`; the running copy is the Verify step's.
//!
//! `cargo test -p orchester-app --test team_qa team_qa`

#[path = "../../examples/qa_app/team/mod.rs"]
mod team;

use orchester_api::test_support::team_server::{TasksAnswer, TeamServer};
use orchester_app::{
  routes::{ProjectPage, Route, TaskList, TaskListTab},
  test_support::{Data, Window, project_id},
};
use orchester_workspace::sharing::project::FirstDownload;
use team::State;

const WRONG_CODE_WORDS: &str = "That code isn't right, or it was used or has expired";
const OFFER: &str = "all-projects-offer";

/// A window on Kontur, with `state` set up for its session, as the QA app opens it.
fn prepared(state: State) -> (Window, TeamServer) {
  let data = Data::new();
  let launch = data.launch();
  data.add_kontur(launch.workspace().expect("the session started"));
  let services = launch.services().expect("the session started");
  let server = team::prepare(state, services, data.runtime.handle()).expect("the state is set up");
  (Window::of(data, launch), server)
}

/// [`prepared`], taken to `state` by a person's presses.
fn driven(state: State) -> (Window, TeamServer) {
  assert!(state.drives());
  let (mut window, server) = prepared(state);
  team::drive::to(&mut window.app, &mut window.tree, (state, &window.link, &server)).expect("the state is reached");
  (window, server)
}

fn tasks_route() -> Route {
  Route::Project(project_id(), ProjectPage::Tasks(TaskList::new(TaskListTab::All)))
}

/// The tasks list, in `state`, on the session's first round.
fn on_the_tasks_list(state: State) -> (Window, TeamServer) {
  assert!(!state.drives());
  let (window, server) = prepared(state);
  window.navigation().go(tasks_route());
  (window, server)
}

fn download(window: &Window) -> Option<FirstDownload> {
  window.projects().feed(project_id()).download.get()
}

#[test]
fn team_qa_join_opens_with_the_address_typed_and_asks_nothing_yet() {
  let (mut window, server) = driven(State::Join);
  let typed = window
    .tree
    .get_element_by_id_mut("orchester-connect-address")
    .and_then(|element| element.as_text_input())
    .map(|input| input.value());
  assert_eq!(typed, Some(server.url()));
  assert!(window.texts("orchester-connect-continue").is_some());
  assert!(window.texts("orchester-connect-join").is_none(), "Continue comes first");
  assert_eq!(server.code_joins(), Vec::<bool>::new());
}

#[test]
fn team_qa_join_refuses_a_wrong_code() {
  let (mut window, server) = driven(State::WrongCode);
  let said = window.text("orchester-connect-join-error");
  assert!(said.contains(WRONG_CODE_WORDS), "{said}");
  assert_eq!(server.code_joins().len(), 1, "the press reached the server");
}

#[test]
fn team_qa_join_refuses_an_expired_code_in_the_same_words() {
  let (mut window, server) = driven(State::ExpiredCode);
  let said = window.text("orchester-connect-join-error");
  assert!(said.contains(WRONG_CODE_WORDS), "{said}");
  assert_eq!(server.code_joins().len(), 1, "the press reached the server");
}

#[test]
fn team_qa_join_says_nothing_answers() {
  let (mut window, server) = driven(State::Unreachable);
  let said = window.text("orchester-connect-address-error");
  assert!(
    said.contains(&format!("Nothing answered at {}", server.url())),
    "{said}"
  );
}

#[test]
fn team_qa_join_with_the_code_joins_and_leads_to_the_teams_projects() {
  let (mut window, server) = driven(State::Joined);
  let said = window.text("orchester-connect-joined");
  assert!(said.contains("Connected to Acme as bohdan"), "{said}");
  assert_eq!(server.code_joins().len(), 1);
  assert!(window.texts("orchester-connect-join-review").is_some());
}

#[test]
fn team_qa_offer_offers_billing_and_says_kontur_is_the_teams() {
  let (mut window, _server) = prepared(State::Offer);
  window.navigation().go(Route::AllProjects);
  window.until("Billing to be offered", |window| window.texts(OFFER).is_some());
  assert_eq!(
    window.text(OFFER),
    "Acme shares Billing with you. It isn't on this computer yet. Open Billing… Not now"
  );
  let place = format!("project-row-where-{}", project_id());
  window.until("where Kontur is", |window| {
    window.texts(&place).is_some_and(|texts| texts == ["Shared · Acme"])
  });
}

#[test]
fn team_qa_downloading_shows_the_skeleton_and_keeps_it_after_a_refusal() {
  let (mut window, server) = on_the_tasks_list(State::Downloading);
  let downloading = Some(FirstDownload::Downloading {
    server: "Acme".to_owned(),
  });
  // The skeleton shows for the list's first reading too: the feed says it now waits for the download.
  let under_way = |window: &mut Window| {
    download(window) == downloading
      && window.texts("tasks-loading").is_some()
      && window.texts("tasks-download-error").is_none()
  };
  window.until("the download's skeleton", under_way);
  window.until("the tasks to be asked for", |_| server.tasks_asked() == 1);
  // A held answer ends in a refusal after a while; here at once, then held again.
  server.answer_tasks(TasksAnswer::Fail);
  window.until("the tasks to be asked for again", |_| server.tasks_asked() >= 2);
  server.answer_tasks(TasksAnswer::Hold);
  let asked = server.tasks_asked();
  window.until("the answer to be held again", |_| server.tasks_asked() > asked);
  window.until("the download to go on", under_way);
}

#[test]
fn team_qa_downloading_shows_the_boards_skeleton_too() {
  let (mut window, _server) = on_the_tasks_list(State::Downloading);
  window.until("the download", |window| download(window).is_some());
  window.until("the layout switch", |window| {
    window.tree.get_element_by_id("tasks-layout-board").is_some()
  });
  window.press("tasks-layout-board");
  // Its bones show after a moment, on a timer the headless window fires only when told.
  window.until("the board's skeleton", |window| {
    window.tree.tick_timers();
    window.tree.get_element_by_id("tasks-loading-board").is_some()
  });
}

#[test]
fn team_qa_download_failed_says_why_and_try_again_downloads_again() {
  let (mut window, server) = on_the_tasks_list(State::DownloadFailed);
  window.until("the failure", |window| window.texts("tasks-download-error").is_some());
  let said = window.text("tasks-download-error");
  assert!(said.contains("Kontur's tasks could not be downloaded"), "{said}");
  assert!(said.contains("Orchester joined Acme"), "{said}");
  assert!(said.contains("The server refused this."), "{said}");
  let asked = server.tasks_asked();
  window.press("tasks-download-error-retry");
  window.until("the tasks to be asked for again", |_| server.tasks_asked() > asked);
  window.until("the failure again", |window| {
    window.texts("tasks-download-error").is_some()
  });
}

#[test]
fn team_qa_a_marker_that_names_nothing_is_an_error() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let database = folder.path().join("orchester.db");
  assert_eq!(State::beside(&database).expect("no marker"), None);
  std::fs::write(database.with_file_name("team-qa"), "team-nothing").expect("a marker");
  let refused = State::beside(&database).expect_err("an unknown state");
  assert!(refused.to_string().contains("team-nothing"), "{refused}");
}
