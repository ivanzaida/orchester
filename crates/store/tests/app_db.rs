use std::{fs, path::Path};

use orchester_core::{
  id::{ProjectId, ServerId, UserId},
  name::Name,
  project::{Project, ProjectKey},
  time::Timestamp,
};
use orchester_store::{AppDb, PreviousSession, ProjectDb, ServerAccount, SessionEnd, StoreError, StoredServer};

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

fn schema_version(path: &Path) -> u32 {
  rusqlite::Connection::open(path)
    .expect("raw connection")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("schema version")
}

/// The schema version a new application database starts at: every migration the build has.
fn current_schema_version() -> u32 {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  drop(AppDb::open(&path).expect("create"));
  schema_version(&path)
}

#[test]
fn a_database_from_a_newer_build_is_refused() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  drop(AppDb::open(&path).expect("create"));
  let current = schema_version(&path);
  rusqlite::Connection::open(&path)
    .expect("raw connection")
    .pragma_update(None, "user_version", current + 1)
    .expect("bump version");
  assert!(matches!(
    AppDb::open(&path),
    Err(StoreError::SchemaTooNew { found, supported }) if found == current + 1 && supported == current
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

/// Registers a project the way schema versions 1 and 2 stored it.
fn insert_project(raw: &rusqlite::Connection, name: &str, repo_root: &str, last_opened_at: i64) {
  raw
    .execute(
      "INSERT INTO projects (id, name, repo_root, added_at, last_opened_at) VALUES (?1, ?2, ?3, 1, ?4)",
      rusqlite::params![ProjectId::generate().to_string(), name, repo_root, last_opened_at],
    )
    .expect("insert a project");
}

#[test]
fn ui_state_and_servers_are_added_to_a_database_of_schema_1() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let raw = AppDb::create_at_version(&path, 1).expect("schema 1");
  insert_project(&raw, "Kontur", "H:/projects/kontur", 1);
  drop(raw);

  let mut db = AppDb::open(&path).expect("migrate");
  db.set_ui_state("tabs", "saved").expect("store");
  db.add_server(&server("https://orchester.kontur.lan", 2))
    .expect("add a server");
  assert_eq!(db.projects().expect("list").len(), 1, "the projects are kept");
  drop(db);
  assert_eq!(schema_version(&path), current_schema_version());
}

#[test]
fn servers_are_added_to_a_database_of_schema_2_and_its_rows_are_kept() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let raw = AppDb::create_at_version(&path, 2).expect("schema 2");
  insert_project(&raw, "Kontur", "H:/projects/kontur", 2);
  insert_project(&raw, "Personal website", "H:/projects/website", 1);
  raw
    .execute_batch(
      "INSERT INTO sessions (pid, started_at, ended_at, end_kind) VALUES (7, 1, 2, 'runs_paused');
       INSERT INTO ui_state (key, value) VALUES ('tabs', 'kontur');",
    )
    .expect("rows of schema 2");
  drop(raw);

  let mut db = AppDb::open(&path).expect("migrate");
  let names: Vec<String> = db
    .projects()
    .expect("list")
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect();
  assert_eq!(names, ["Kontur", "Personal website"]);
  assert_eq!(db.ui_state("tabs").expect("read").as_deref(), Some("kontur"));
  let (_, previous) = db.start_session(8, at(3)).expect("start");
  assert_eq!(previous, PreviousSession::Ended(SessionEnd::RunsPaused));
  assert_eq!(db.servers().expect("list"), []);
  let kontur = server("https://orchester.kontur.lan", 4);
  db.add_server(&kontur).expect("add a server");
  assert_eq!(db.servers().expect("list"), [kontur]);
  drop(db);
  assert_eq!(schema_version(&path), current_schema_version());
}

fn server(url: &str, added_at: i64) -> StoredServer {
  StoredServer {
    id: ServerId::generate(),
    url: url.to_owned(),
    name: "Kontur team".to_owned(),
    plain_http_allowed: false,
    added_at: at(added_at),
    account: None,
  }
}

#[test]
fn servers_keep_their_address_name_opt_in_and_account() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  let kontur = server("https://orchester.kontur.lan", 1);
  let lab = StoredServer {
    plain_http_allowed: true,
    ..server("http://10.0.4.20:8740", 2)
  };
  db.add_server(&kontur).expect("add Kontur team");
  db.add_server(&lab).expect("add the lab");
  let account = ServerAccount {
    user: UserId::generate(),
    username: "bohdan".to_owned(),
    display_name: "Bohdan Koval".to_owned(),
    admin: true,
    signed_in_at: at(3),
  };
  db.set_server_account(kontur.id, Some(&account)).expect("sign in");
  db.set_server_name(kontur.id, "Kontur").expect("rename");
  drop(db);

  let mut db = AppDb::open(&path).expect("reopen");
  let servers = db.servers().expect("list");
  assert_eq!(servers.len(), 2);
  assert_eq!(servers[0].url, "https://orchester.kontur.lan");
  assert_eq!(servers[0].name, "Kontur");
  assert_eq!(servers[0].account.as_ref(), Some(&account));
  assert!(servers[1].plain_http_allowed);
  assert_eq!(servers[1].account, None);

  db.set_server_account(kontur.id, None).expect("sign out");
  assert_eq!(db.servers().expect("list")[0].account, None);
}

#[test]
fn servers_refuse_an_address_twice_and_an_unknown_server() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  db.add_server(&server("https://orchester.kontur.lan", 1)).expect("add");
  assert!(db.add_server(&server("https://orchester.kontur.lan", 2)).is_err());
  let unknown = ServerId::generate();
  assert!(matches!(
    db.set_server_name(unknown, "Lab"),
    Err(StoreError::ServerNotFound(id)) if id == unknown
  ));
  assert_eq!(db.servers().expect("list").len(), 1);
}
