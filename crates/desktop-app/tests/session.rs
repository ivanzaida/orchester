//! Starting the app on a data folder: the session is recorded in its application database and ends
//! quietly; a second app in the same folder is refused and leaves the database alone; a session that
//! never recorded its end is reported crashed to the next one; and a database that cannot be opened
//! shows the startup screen that says why, with its path.

mod support;

use std::fs;

use orchester_app::{
  root::Start,
  startup::{Failure, Problem, StartupProblem},
};
use orchester_core::time::Timestamp;
use orchester_ipc::SessionEnd as AttachEnd;
use orchester_store::{AppDb, PreviousSession, SessionEnd, location::LockHolder};
use support::{Data, Window};

/// How the last session of `data` ended, as the next one would find it. Records a session itself.
fn last_session(data: &Data) -> PreviousSession {
  let mut db = AppDb::open(&data.database()).expect("the database opens");
  let (_, previous) = db.start_session(1, Timestamp::now()).expect("a session starts");
  previous
}

fn problem(start: Start) -> StartupProblem {
  match start {
    Start::Problem(Problem { problem, .. }) => problem,
    Start::Ready(_) => panic!("the session started"),
  }
}

#[test]
fn session_records_a_session_and_its_quiet_end() {
  let data = Data::new();
  let launch = data.launch();
  let services = launch.services().expect("the session started");
  assert_eq!(services.previous_end, AttachEnd::Clean, "nothing came before it");
  launch.end();
  assert_eq!(last_session(&data), PreviousSession::Ended(SessionEnd::Clean));
}

#[test]
fn session_second_start_in_the_same_folder_is_in_use() {
  let data = Data::new();
  let first = data.launch();
  let first_start = first.services().expect("the first app started").session_start;

  let second = data.launch();
  assert!(second.workspace().is_none(), "the second app opens no database");
  let StartupProblem::AlreadyOpen { folder, holder } = problem(second.start()) else {
    panic!("the second app is told the folder is in use");
  };
  assert_eq!(folder, data.dir.path());
  let holder = holder.expect("the first app's record says who it is");
  assert_eq!(holder.pid, std::process::id());
  assert_eq!(holder.started_at, first_start);

  second.end();
  first.end();
  assert_eq!(
    last_session(&data),
    PreviousSession::Ended(SessionEnd::Clean),
    "the second app recorded nothing that would read as a crash"
  );
}

#[test]
fn session_already_open_screen_names_the_folder_and_the_other_window() {
  let data = Data::new();
  let first = data.launch();
  let second = data.launch();
  let folder = data.dir.path().display().to_string();
  let mut window = Window::of(data, second);
  let texts = window
    .texts("startup-already-open")
    .expect("the already-open screen shows");
  assert_eq!(texts[0], "Orchester is already open");
  assert!(texts.contains(&folder), "{texts:?}");
  let other = format!("· process {}", std::process::id());
  assert!(
    texts
      .iter()
      .any(|text| text.starts_with("Open since ") && text.ends_with(&other)),
    "{texts:?}"
  );
  assert!(window.texts("startup-close").is_some(), "the window can be closed");
  assert_eq!(
    window.texts("status-bar").expect("a status bar"),
    [format!("v{}", env!("CARGO_PKG_VERSION"))],
    "only the version"
  );
  assert!(window.texts("tab-strip").is_none(), "no tabs");
  assert_eq!(window.link.startup(), Some("already_open"));
  first.end();
}

/// Rewrites the record of who holds `data`'s folder, as the app that holds it would.
fn record_holder(data: &Data, holder: LockHolder) {
  let text = serde_json::to_string(&holder).expect("a holder serializes");
  fs::write(data.dir.path().join("orchester.lock.holder"), text).expect("the record is written");
}

#[test]
fn session_closing_app_says_to_start_again_never_that_it_is_stuck() {
  let data = Data::new();
  let first = data.launch();
  let started = first.services().expect("the first app started").session_start;
  record_holder(
    &data,
    LockHolder {
      closing: true,
      ..LockHolder::running(std::process::id(), started)
    },
  );
  let second = data.launch();
  let mut window = Window::of(data, second);
  let texts = window.texts("startup-closing").expect("the closing screen shows");
  assert_eq!(texts[0], "Orchester is closing");
  assert!(window.texts("startup-switch").is_none(), "nothing to switch to");
  assert!(window.texts("startup-no-answer").is_none());
  assert!(window.texts("startup-close").is_some());
  assert_eq!(window.link.startup(), Some("closing"));
  drop(window);
  first.end();
}

#[test]
fn session_other_app_from_another_day_says_the_day() {
  let data = Data::new();
  let first = data.launch();
  let two_days_ago = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 2 * 24 * 60 * 60 * 1000);
  record_holder(&data, LockHolder::running(18244, two_days_ago));
  let second = data.launch();
  let mut window = Window::of(data, second);
  let other = window
    .texts("startup-other-window")
    .expect("the other window's line shows");
  assert!(
    other[0].starts_with("Open since ") && other[0].contains(", ") && other[0].ends_with("· process 18244"),
    "{other:?}"
  );
  drop(window);
  first.end();
}

#[test]
fn session_that_cannot_land_ends_cleanly() {
  let data = Data::new();
  data.launch().end();
  {
    let connection = rusqlite::Connection::open(data.database()).expect("the database opens");
    connection
      .execute(
        "INSERT INTO projects (id, name, repo_root, added_at, last_opened_at) VALUES ('not an id', '', 'x', 1, 1)",
        [],
      )
      .expect("a project the list cannot read");
  }
  let launch = data.launch();
  assert!(
    matches!(
      problem(launch.start()),
      StartupProblem::Failed(Failure::Unreadable { .. })
    ),
    "the list of projects cannot be read"
  );
  assert_eq!(
    last_session(&data),
    PreviousSession::Ended(SessionEnd::Clean),
    "the session that could not open ended cleanly, not as a crash"
  );
}

#[test]
fn session_unended_previous_is_crashed() {
  let data = Data::new();
  drop(data.launch());
  let next = data.launch();
  assert_eq!(
    next.services().expect("the session started").previous_end,
    AttachEnd::Crashed
  );
}

#[test]
fn session_unreadable_database_shows_the_failure_screen_with_its_path() {
  let data = Data::new();
  fs::write(data.database(), "not an Orchester database, nor any other").expect("the file is written");
  let launch = data.launch();
  let StartupProblem::Failed(Failure::Unreadable { database, reason }) = problem(launch.start()) else {
    panic!("the database cannot be read");
  };
  assert_eq!(database, data.database());
  assert!(reason.contains("not a database"), "the system's words: {reason}");

  let path = data.database().display().to_string();
  let mut window = Window::of(data, launch);
  let texts = window.texts("startup-unreadable").expect("the failure screen shows");
  assert_eq!(texts[0], "Orchester can't read its data");
  assert!(texts.contains(&path), "the database's path: {texts:?}");
  assert!(texts.contains(&"Nothing was changed.".to_owned()), "{texts:?}");
  assert!(window.texts("startup-show-folder").is_some());
  assert_eq!(window.link.startup(), Some("unreadable"));
}

#[test]
fn session_newer_data_says_both_versions() {
  let data = Data::new();
  {
    let connection = rusqlite::Connection::open(data.database()).expect("a database");
    connection
      .pragma_update(None, "user_version", 999)
      .expect("a newer schema version");
  }
  let launch = data.launch();
  let StartupProblem::Failed(Failure::NewerData { found, supported, .. }) = problem(launch.start()) else {
    panic!("the database was written by a newer build");
  };
  assert_eq!(found, 999);
  let mut window = Window::of(data, launch);
  let texts = window.texts("startup-newer-data").expect("the failure screen shows");
  assert!(
    texts.contains(&format!(
      "It has data version 999; this build reads up to version {supported}. Nothing was changed."
    )),
    "{texts:?}"
  );
}
