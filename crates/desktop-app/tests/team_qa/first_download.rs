//! *Download tasks*, the last step of the start page (`YN6MO`), on the QA app's team states: 84 of 212 tasks down, each
//! part with its state; *Open Kontur* before the end, with the tasks going on arriving in its list; a failure in place
//! with *Try again*; *Stay here*. `cargo test -p orchester-app --test team_qa team_qa_first_download`

use orchester_api::test_support::team_server::{TasksAnswer, TeamServer};
use orchester_app::{
  routes::{ProjectPage, Route, TaskList},
  test_support::{Window, project_id},
};

use super::{prepared, team::State};

const COUNT: &str = "first_download-tasks-sub";
const OPEN: &str = "first_download-open";
const CAN_OPEN: &str = "first_download-can-open";
const STAY: &str = "first_download-stay";
const FAILED: &str = "first_download-tasks-reason";
const RETRY: &str = "first_download-tasks-retry";

/// The step on `state`, once 84 of the tasks are down.
fn at_84(state: State) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  window.navigation().go(Route::FirstDownload(project_id()));
  window.until("84 of 212", |window| {
    window.texts(COUNT).is_some_and(|count| count.concat() == "84 of 212")
  });
  (window, server)
}

fn width(window: &mut Window, id: &str) -> f32 {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .width
}

#[test]
fn team_qa_first_download_shows_each_part_with_the_count_and_the_latest_arrivals() {
  let (mut window, _server) = at_84(State::DownloadProgress);
  window.until("the part's words", |window| {
    window.texts("first_download-joined-title").is_some()
      && window.text("first_download-tasks-title") == "Downloading tasks"
  });
  assert_eq!(window.text("first_download-title"), "Downloading Kontur's tasks");
  assert_eq!(window.text("first_download-joined-title"), "Joined Acme");
  assert!(
    window.text("first_download-linked-title").starts_with("Linked to "),
    "{}",
    window.text("first_download-linked-title")
  );
  window.until("the settings' words", |window| {
    window.text("first_download-settings-title") == "Team settings"
  });
  for (key, title, status) in [
    ("KON-84", "Retry webhook delivery on 5xx", "Todo"),
    ("KON-83", "Show the signed-in account in the menu", "In progress"),
    ("KON-82", "Export invoices as PDF", "In review"),
  ] {
    let row = format!("first_download-tasks-arrival-{key}");
    window.until(&row, |window| window.texts(&row).is_some());
    assert_eq!(window.text(&format!("{row}-key")), key);
    assert_eq!(window.text(&format!("{row}-title")), title);
    assert_eq!(window.text(&format!("{row}-chip-label")), status);
  }
  let done = width(&mut window, "first_download-tasks-bar-done") / width(&mut window, "first_download-tasks-bar");
  assert!((done - 84.0 / 212.0).abs() < 0.01, "the bar is the real share: {done}");
  assert_eq!(window.text(OPEN), "Open Kontur");
}

/// Makes Kontur's checkout a git repository whose `origin` is `url`.
fn give_origin(window: &Window, url: &str) {
  let repo = window.data.dir.path().join("repo-kontur");
  for args in [&["init", "--quiet"][..], &["remote", "add", "origin", url][..]] {
    let status = std::process::Command::new("git")
      .args(args)
      .current_dir(&repo)
      .status()
      .expect("git runs");
    assert!(status.success(), "git {args:?}");
  }
}

/// Waits for what the step says of the checkout's `origin` to be `words`.
fn says_of_origin(window: &mut Window, words: &str) {
  window.navigation().go(Route::FirstDownload(project_id()));
  window.until(words, |window| {
    window
      .texts("first_download-linked-sub")
      .is_some_and(|sub| sub.concat() == words)
  });
}

#[test]
fn team_qa_first_download_says_the_origin_matches_the_projects_repository() {
  let (mut window, _server) = prepared(State::DownloadProgress);
  give_origin(&window, "https://github.com/acme/kontur.git");
  says_of_origin(&mut window, "origin github.com/acme/kontur matches the project");
}

#[test]
fn team_qa_first_download_says_when_the_origin_is_another_repository() {
  let (mut window, _server) = prepared(State::DownloadProgress);
  give_origin(&window, "https://github.com/acme/other.git");
  says_of_origin(
    &mut window,
    "origin github.com/acme/other isn't the project's github.com/acme/kontur",
  );
}

#[test]
fn team_qa_first_download_open_works_before_the_end_and_the_list_keeps_filling() {
  let (mut window, server) = at_84(State::DownloadProgress);
  window.press(OPEN);
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(project_id(), ProjectPage::Tasks(TaskList::default()))
  );
  let count = |window: &mut Window, tasks: &str| {
    window
      .texts("tasks-view-all-count")
      .is_some_and(|count| count.concat() == tasks)
  };
  window.until("the 84 tasks down in the list", |window| count(window, "84"));
  server.answer_tasks(TasksAnswer::Serve);
  window.until_within(
    std::time::Duration::from_secs(30),
    "the 212 tasks in the list",
    |window| count(window, "212"),
  );
}

#[test]
fn team_qa_first_download_a_failure_shows_in_place_and_try_again_goes_on() {
  let (mut window, server) = at_84(State::DownloadStopped);
  window.until("the failure", |window| window.texts(FAILED).is_some());
  assert_eq!(window.text("first_download-tasks-title"), "Downloading tasks stopped");
  assert_eq!(window.text(COUNT), "84 of 212", "what came down stays");
  assert_eq!(window.text(FAILED), "The server refused this.");
  assert!(
    window.texts(CAN_OPEN).is_none(),
    "it no longer says tasks keep arriving"
  );
  assert!(window.texts(OPEN).is_some(), "the project still opens");

  server.answer_tasks(TasksAnswer::Serve);
  window.press(RETRY);
  window.until_within(std::time::Duration::from_secs(30), "the end", |window| {
    window
      .texts("first_download-tasks-title")
      .is_some_and(|title| title.concat() == "Tasks downloaded")
  });
  assert_eq!(window.text(COUNT), "212 of 212");
  assert!(window.texts(FAILED).is_none());
  assert!(window.texts(STAY).is_none(), "nothing to stay for once it ended");
}

#[test]
fn team_qa_first_download_a_failure_before_the_first_page_asks_nobody() {
  let (mut window, server) = prepared(State::DownloadFailed);
  window.navigation().go(Route::FirstDownload(project_id()));
  window.until("the failure", |window| window.texts(FAILED).is_some());
  assert_eq!(window.text("first_download-tasks-title"), "Downloading tasks stopped");
  assert_eq!(window.text(COUNT), "No tasks arrived");
  assert_eq!(window.text(FAILED), "The server refused this.");
  assert!(window.texts(CAN_OPEN).is_none());

  server.answer_tasks(TasksAnswer::Hold);
  window.press(RETRY);
  window.until("the server to be asked again", |window| {
    window
      .texts(COUNT)
      .is_some_and(|count| count.concat() == "Asking Acme for them…")
  });
  assert_eq!(window.text("first_download-tasks-title"), "Downloading tasks");
  assert!(window.texts(FAILED).is_none());
}

#[test]
fn team_qa_first_download_stay_here_puts_away_what_says_it_can_open() {
  let (mut window, _server) = at_84(State::DownloadProgress);
  window.until("the callout", |window| window.texts(CAN_OPEN).is_some());
  assert_eq!(
    window.text(&format!("{CAN_OPEN}-title")),
    "You can open Kontur now",
    "it says the project can open"
  );
  window.press(STAY);
  window.until("the callout to go", |window| window.texts(CAN_OPEN).is_none());
  assert_eq!(
    window.navigation().route_now(),
    Route::FirstDownload(project_id()),
    "the page stays"
  );
  assert!(window.texts(OPEN).is_some());
}
