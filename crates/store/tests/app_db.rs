use std::path::Path;

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{AppDb, PreviousSession, SessionEnd};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn project(name: &str) -> Project {
  Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new(name).expect("valid name"),
  )
}

#[test]
fn sessions_report_clean_ends_and_crashes_once() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");

  let mut db = AppDb::open(&path).expect("open");
  let (first, previous) = db.start_session(1, at(1)).expect("start");
  assert_eq!(previous, PreviousSession::None);
  db.end_session(first, SessionEnd::RunsPaused, at(2)).expect("end");

  let (_crashing, previous) = db.start_session(2, at(3)).expect("start");
  assert_eq!(previous, PreviousSession::Ended(SessionEnd::RunsPaused));
  drop(db);

  let mut db = AppDb::open(&path).expect("reopen");
  let (_, previous) = db.start_session(3, at(4)).expect("start after crash");
  assert_eq!(previous, PreviousSession::Crashed);
}

#[test]
fn projects_are_registered_once_and_listed_by_last_use() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let kontur = project("Kontur");
  let website = project("Personal website");
  db.register_project(&kontur, Path::new("H:/projects/kontur"), at(1))
    .expect("register");
  db.register_project(&website, Path::new("H:/projects/website"), at(2))
    .expect("register");
  db.register_project(&kontur, Path::new("H:/projects/kontur"), at(3))
    .expect("reopen");

  let projects = db.projects().expect("list");
  let names: Vec<&str> = projects.iter().map(|project| project.name.as_str()).collect();
  assert_eq!(names, ["Kontur", "Personal website"]);
  assert_eq!(projects[0].last_opened_at, at(3));
}

#[test]
fn a_database_from_a_newer_build_is_refused() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  drop(AppDb::open(&path).expect("create"));
  rusqlite::Connection::open(&path)
    .expect("raw connection")
    .pragma_update(None, "user_version", 99)
    .expect("bump version");
  assert!(matches!(
    AppDb::open(&path),
    Err(orchester_store::StoreError::SchemaTooNew {
      found: 99,
      supported: 1
    })
  ));
}
