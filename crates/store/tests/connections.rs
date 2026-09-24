use std::{fs, path::PathBuf};

use orchester_core::{
  connection::{
    AgentProvider, CatalogModel, CatalogSource, CheckResult, Connection, ConnectionCheck, ConnectionStatus,
    ModelCatalog, SignIn,
  },
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection as Sqlite;
use tempfile::TempDir;

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn executable(repo: &TempDir, file: &str) -> PathBuf {
  repo.path().join("bin").join(file)
}

fn connection(repo: &TempDir, name: &str, provider: AgentProvider) -> Connection {
  Connection::new(
    ConnectionId::generate(),
    Name::new(name).expect("valid name"),
    provider,
    executable(repo, "codex.exe"),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("absolute executable")
}

fn available() -> ConnectionCheck {
  let model = CatalogModel::new(
    ModelId::new("gpt-5.5").expect("valid model"),
    vec!["low".into(), "high".into()],
  );
  ConnectionCheck {
    at: Timestamp::from_unix_millis(2_000),
    result: CheckResult::Available {
      version: "0.139.0".into(),
      sign_in: SignIn::SignedIn,
      catalog: ModelCatalog::new(CatalogSource::Provider, vec![model]),
    },
  }
}

#[test]
fn connection_round_trip_keeps_the_check() {
  let (repo, mut db) = project_repo();
  let mut codex = connection(&repo, "Codex", AgentProvider::CodexCli);
  db.create_connection(&codex).expect("create");
  assert_eq!(db.connection(codex.id()).expect("read"), Some(codex.clone()));

  codex.record_check(available());
  codex.rename(Name::new("Codex · Kontur").expect("valid name"));
  db.save_connection(&codex).expect("save");
  let stored = db.connection(codex.id()).expect("read").expect("exists");
  assert_eq!(stored, codex);
  assert_eq!(stored.status(), ConnectionStatus::Connected);
  assert_eq!(stored.models()[0].efforts(), ["low", "high"]);

  let reopened = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(reopened.connections().expect("list"), [codex]);
}

#[test]
fn connection_names_are_unique_and_listed_in_order() {
  let (repo, mut db) = project_repo();
  let codex = connection(&repo, "Codex", AgentProvider::CodexCli);
  let claude = connection(&repo, "Claude", AgentProvider::ClaudeCodeCli);
  db.create_connection(&codex).expect("create codex");
  db.create_connection(&claude).expect("create claude");
  let names: Vec<String> = db
    .connections()
    .expect("list")
    .iter()
    .map(|connection| connection.name().as_str().to_owned())
    .collect();
  assert_eq!(names, ["Claude", "Codex"]);

  let duplicate = connection(&repo, "Codex", AgentProvider::ClaudeCodeCli);
  assert!(matches!(
    db.create_connection(&duplicate),
    Err(StoreError::DuplicateConnectionName(name)) if name == "Codex"
  ));
  let mut renamed = claude;
  renamed.rename(Name::new("Codex").expect("valid name"));
  assert!(matches!(
    db.save_connection(&renamed),
    Err(StoreError::DuplicateConnectionName(_))
  ));
}

#[test]
fn saving_an_unknown_connection_fails() {
  let (repo, mut db) = project_repo();
  let codex = connection(&repo, "Codex", AgentProvider::CodexCli);
  assert!(matches!(
    db.save_connection(&codex),
    Err(StoreError::ConnectionNotFound(id)) if id == codex.id()
  ));
}

#[test]
fn connection_rows_hold_no_credentials_and_a_status() {
  let (repo, mut db) = project_repo();
  let mut codex = connection(&repo, "Codex", AgentProvider::CodexCli);
  codex.record_check(available());
  db.create_connection(&codex).expect("create");
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let (kind, status, settings): (String, String, String) = raw
    .query_row("SELECT kind, status, settings FROM connections", [], |row| {
      Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .expect("row");
  assert_eq!(kind, "agent_provider");
  assert_eq!(status, "connected");
  let settings: serde_json::Value = serde_json::from_str(&settings).expect("JSON settings");
  let keys: Vec<&String> = settings["body"].as_object().expect("settings body").keys().collect();
  assert_eq!(keys, ["executable"]);
}

#[test]
fn damaged_connection_rows_are_reported() {
  let (repo, mut db) = project_repo();
  let codex = connection(&repo, "Codex", AgentProvider::CodexCli);
  db.create_connection(&codex).expect("create");
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  raw
    .execute("UPDATE connections SET provider = 'gemini'", [])
    .expect("damage the row");
  assert!(matches!(
    db.connections(),
    Err(StoreError::Corrupt {
      what: "connection provider",
      ..
    })
  ));
}
