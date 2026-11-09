//! The QA app's team scenarios (`examples/qa_app/team`), each state on the headless window with the stand-in team
//! server: what the person's presses reach on the start page's *Join a team* with a code, and what the session's own
//! work reaches with Kontur linked and joined; a team project's Tasks list read from the server is in [`team_tasks`],
//! its Task page, Modules list and Module page in [`team_pages`]. Nothing here starts `tools/qa-app.ps1`; the running
//! copy is the Verify step's.
//!
//! `cargo test -p orchester-app --test team_qa team_qa`

mod first_download;
mod hub_archive;
mod hub_conflict;
mod hub_created;
mod hub_edit;
mod hub_edit_race;
mod hub_unsent;
mod hub_unsent_publish;
#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
#[allow(
  clippy::duplicate_mod,
  reason = "the seed's team scenarios and the QA app's team states both include the state file (the marker the seed writes and the QA app reads)"
)]
mod kontur;
mod not_ready;
mod start_failed;
mod start_notices;
#[path = "../../examples/qa_app/team/mod.rs"]
mod team;
mod team_actions;
mod team_create;
mod team_edit;
mod team_lifecycle;
mod team_pages;
mod team_tasks;
mod team_work_actions;

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::{TasksAnswer, TeamServer};
use orchester_app::{
  routes::{JoinWay, ProjectPage, Route, TaskList, TaskListTab},
  test_support::{Data, Window, project_id},
};
use team::State;

const ADDRESS: &str = "join-team-code-address";
const CODE_FIELD: &str = "join-team-code-code";
const SUBMIT: &str = "join-team-code-submit";
const WRONG: &str = "join-team-code-wrong";
const WRONG_CODE_WORDS: &str = "Wrong or expired. A code works once, within 10 minutes: ask for a new one.";
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

fn value(window: &mut Window, id: &str) -> String {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .value()
}

#[test]
fn team_qa_join_opens_on_the_code_with_the_address_typed_and_takes_no_mcp_press() {
  let (mut window, server) = driven(State::Join);
  assert_eq!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Code));
  assert_eq!(value(&mut window, ADDRESS), server.url());
  assert_eq!(value(&mut window, CODE_FIELD), "");

  with_synthetic_input(|| window.press(SUBMIT));
  let refused = format!("{SUBMIT}-person-only");
  window.until("the press to be refused", |window| window.texts(&refused).is_some());
  assert_eq!(server.code_joins(), Vec::<bool>::new());
}

#[test]
fn team_qa_join_refuses_a_wrong_code() {
  let (mut window, server) = driven(State::WrongCode);
  assert_eq!(window.text(WRONG), WRONG_CODE_WORDS);
  assert_eq!(server.code_joins().len(), 1, "the press reached the server");
  assert_eq!(value(&mut window, CODE_FIELD), "WRONG123");
}

#[test]
fn team_qa_join_refuses_an_expired_code_in_the_same_words() {
  let (mut window, server) = driven(State::ExpiredCode);
  assert_eq!(window.text(WRONG), WRONG_CODE_WORDS);
  assert_eq!(server.code_joins().len(), 1, "the press reached the server");
}

#[test]
fn team_qa_join_says_it_cannot_reach_the_server_and_keeps_what_was_typed() {
  let (mut window, server) = driven(State::Unreachable);
  assert_eq!(
    window.text("join-team-code-unreachable-title"),
    format!("Can't reach {}", server.url())
  );
  assert!(window.texts("join-team-code-try-again").is_some());
  assert_eq!(value(&mut window, ADDRESS), server.url());
  assert!(!value(&mut window, CODE_FIELD).is_empty());
  assert_eq!(server.code_joins(), Vec::<bool>::new());
}

#[test]
fn team_qa_join_with_the_code_joins_and_leads_to_the_teams_projects() {
  let (mut window, server) = driven(State::Joined);
  assert_eq!(window.text("join-team-code-joined-title"), "Joined Acme as bohdan");
  assert_eq!(server.code_joins().len(), 1);

  window.press("join-team-code-open");
  assert!(
    matches!(window.navigation().route_now(), Route::JoinTeam(JoinWay::Projects(_))),
    "See your projects there opens the team's shared projects"
  );
  window.until("the team's projects", |window| {
    window.texts("join-team-projects-kontur").is_some()
  });
}

/// The team's projects, in `state`, with the team joined.
fn at_the_teams_projects(state: State) -> (Window, TeamServer) {
  let (window, server) = prepared(state);
  let joined = window
    .launch
    .services()
    .expect("the session started")
    .servers
    .entries()
    .expect("the servers are read")
    .into_iter()
    .find(|entry| entry.in_use())
    .expect("the team was joined");
  window
    .navigation()
    .go(Route::JoinTeam(JoinWay::Projects(joined.stored.id)));
  (window, server)
}

#[test]
fn team_qa_shared_projects_lists_the_team_with_hints_and_an_admin() {
  let (mut window, _server) = at_the_teams_projects(State::SharedProjects);
  window.until("Billing", |window| window.texts("join-team-projects-billing").is_some());
  assert_eq!(window.text("join-team-projects-billing-role"), "you: Admin");
  assert_eq!(window.text("join-team-projects-kontur-role"), "you: member");
}

#[test]
fn team_qa_shared_none_says_who_to_ask() {
  let (mut window, _server) = at_the_teams_projects(State::NoSharedProjects);
  window.until("the empty state", |window| {
    window.texts("join-team-projects-none").is_some()
  });
  assert_eq!(
    window.text("join-team-projects-none-title"),
    "No projects for you on Acme yet"
  );
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

/// A team project's list reads its tasks from the server (ORC-716): while the server holds them, the skeleton, and
/// nothing this computer kept; once it refuses them, that team data is unavailable.
#[test]
fn team_qa_downloading_shows_the_skeleton_while_the_server_holds_the_tasks() {
  let (mut window, server) = on_the_tasks_list(State::Downloading);
  window.until("the skeleton", |window| window.texts("tasks-loading").is_some());
  window.until("the tasks to be asked for", |_| server.authority_reads() >= 1);
  assert!(window.texts("task-row-KON-1").is_none(), "nothing read before shows");
  server.answer_tasks(TasksAnswer::Fail);
  window.until("the outage", |window| window.texts("tasks-team-unavailable").is_some());
  assert!(window.texts("tasks-loading").is_none());
}

#[test]
fn team_qa_downloading_shows_the_boards_skeleton_too() {
  let (mut window, _server) = on_the_tasks_list(State::Downloading);
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
fn team_qa_download_failed_says_team_data_is_unavailable_and_try_again_reads_again() {
  let (mut window, server) = on_the_tasks_list(State::DownloadFailed);
  window.until("the outage", |window| window.texts("tasks-team-unavailable").is_some());
  let said = window.text("tasks-team-unavailable");
  assert!(said.contains("Team data is unavailable"), "{said}");
  let host = server.url().trim_start_matches("http://").to_owned();
  assert!(said.contains(&format!("{host} hasn't answered since")), "{said}");
  let reads = server.authority_reads();
  window.press("tasks-team-try-again");
  window.until("the tasks to be read again", |_| server.authority_reads() > reads);
  window.until("the outage again", |window| {
    window.texts("tasks-team-unavailable").is_some()
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
