use std::{fs, path::Path};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{AppDb, PreviousSession, ProjectDb, SessionEnd};

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
      supported: 2
    })
  ));
}

#[test]
fn projects_forgotten_leave_their_folder_and_data_on_disk() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let repo = dir.path().join("kontur");
  fs::create_dir_all(repo.join(".git")).expect("create .git");
  let kontur = project("Kontur");
  drop(ProjectDb::create(&repo, kontur.clone()).expect("create the project"));
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  db.register_project(&kontur, &repo, at(1)).expect("register");
  db.register_project(&project("Personal website"), Path::new("H:/projects/website"), at(2))
    .expect("register");

  db.forget_project(kontur.id()).expect("forget");

  let names: Vec<String> = db
    .projects()
    .expect("list")
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect();
  assert_eq!(names, ["Personal website"]);
  assert!(
    repo.join(".orchester").join("orchester.db").is_file(),
    "the project data stays"
  );
  let reopened = ProjectDb::open(&repo).expect("the project opens again");
  assert_eq!(reopened.project().id(), kontur.id());
  // Forgetting it again changes nothing.
  db.forget_project(kontur.id()).expect("forget again");
}

#[test]
fn projects_whose_folder_is_gone_are_listed_as_missing() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let (here, gone) = (dir.path().join("kontur"), dir.path().join("ledger"));
  fs::create_dir_all(&here).expect("create the folder");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  db.register_project(&project("Kontur"), &here, at(2)).expect("register");
  db.register_project(&project("Ledger"), &gone, at(1)).expect("register");

  let listed: Vec<(String, bool)> = db
    .projects()
    .expect("list")
    .iter()
    .map(|project| (project.name.as_str().to_owned(), project.folder_missing()))
    .collect();
  assert_eq!(
    listed,
    [("Kontur".to_owned(), false), ("Ledger".to_owned(), true)],
    "a project whose folder is gone is still listed"
  );
}

#[test]
fn ui_state_keeps_the_latest_value_per_key_across_opens() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  assert_eq!(db.ui_state("tabs").expect("read"), None);
  db.set_ui_state("tabs", "first").expect("store");
  db.set_ui_state("tabs", "second").expect("replace");
  db.set_ui_state("other", "kept apart").expect("store another key");
  drop(db);

  let db = AppDb::open(&path).expect("reopen");
  assert_eq!(db.ui_state("tabs").expect("read").as_deref(), Some("second"));
  assert_eq!(db.ui_state("other").expect("read").as_deref(), Some("kept apart"));
}

#[test]
fn ui_state_is_added_to_a_database_of_the_previous_version() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("create");
  let kontur = project("Kontur");
  db.register_project(&kontur, Path::new("H:/projects/kontur"), at(1))
    .expect("register");
  drop(db);
  // As the previous version left it: no ui_state table, schema version 1.
  let raw = rusqlite::Connection::open(&path).expect("raw connection");
  raw.execute_batch("DROP TABLE ui_state").expect("drop the table");
  raw.pragma_update(None, "user_version", 1).expect("set the version");
  drop(raw);

  let mut db = AppDb::open(&path).expect("migrate");
  db.set_ui_state("tabs", "saved").expect("store");
  assert_eq!(db.projects().expect("list").len(), 1, "the projects are kept");
}
