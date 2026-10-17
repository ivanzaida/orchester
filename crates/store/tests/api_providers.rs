//! Agent CLI connections on API providers (`docs/design/api-providers.md`): the provider is kept in its own column,
//! with the key secret's id and never a value, and a database from before the column opens with every connection on
//! its CLI's own sign-in.
//!
//! `cargo test -p orchester-store --test api_providers api_providers`

mod support;

use std::fs;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiProvider, BaseUrl, CatalogSource, CheckResult, ConnectionCheck,
    ConnectionStatus, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ProjectId, SecretId},
  project::{Project, ProjectKey},
  resource::ModelId,
  store::ConnectionStore,
};
use orchester_store::ProjectDb;
use rusqlite::Connection as Sqlite;
use support::{at, copied_at_version, name};
use tempfile::TempDir;

/// The schema before connections kept an API provider (`api_providers`, the 72nd). Migrations are append-only.
const BEFORE_API_PROVIDERS: u32 = 71;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("project");
  (repo, db)
}

fn claude(repo: &TempDir) -> AgentConnection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name("DeepSeek · Claude Code"),
    AgentProvider::ClaudeCodeCli,
    repo.path().join("bin").join("claude.exe"),
    at(1_000),
  )
  .expect("an absolute program");
  connection
    .record_check(ConnectionCheck {
      at: at(2_000),
      result: CheckResult::Available {
        version: "2.1.301".into(),
        sign_in: SignIn::SignedOut,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("a CLI check");
  connection
}

fn deepseek(key: SecretId) -> ApiProvider {
  ApiProvider::new(
    BaseUrl::new("https://api.deepseek.com/anthropic").expect("valid"),
    key,
    vec![ModelId::new("deepseek-flash").expect("valid")],
  )
  .expect("a model")
}

#[test]
fn api_providers_round_trip_in_their_own_column_with_the_secret_id_only() {
  let (repo, mut db) = project();
  let key = SecretId::generate();
  let mut connection = claude(&repo);
  connection.set_api_provider(Some(deepseek(key))).expect("on Default");
  db.create_connection(&connection).expect("create");

  let stored = AgentConnection::try_from(db.connection(connection.id()).expect("read").expect("exists"))
    .expect("an agent connection");
  assert_eq!(stored, connection);
  assert_eq!(stored.status(), ConnectionStatus::Connected);
  drop(db);

  let raw = Sqlite::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  let (provider, status): (String, String) = raw
    .query_row(
      "SELECT api_provider, status FROM connections WHERE id = ?1",
      [connection.id().to_string()],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("the row");
  let provider: serde_json::Value = serde_json::from_str(&provider).expect("JSON");
  assert_eq!(
    provider,
    serde_json::json!({"schema": 1, "body": {
      "base_url": "https://api.deepseek.com/anthropic",
      "key": key.to_string(),
      "models": ["deepseek-flash"],
    }})
  );
  assert_eq!(status, "connected");
  drop(raw);

  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  let mut back = stored;
  back.set_api_provider(None).expect("back to the sign-in");
  db.save_connection(&back).expect("save");
  let reread = db.connection(back.id()).expect("read").expect("exists");
  assert!(reread.api_provider().is_none());
  assert_eq!(reread.status(), ConnectionStatus::SignInRequired);
}

#[test]
fn api_providers_a_damaged_provider_is_reported() {
  let (repo, mut db) = project();
  let connection = claude(&repo);
  db.create_connection(&connection).expect("create");
  drop(db);
  let raw = Sqlite::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  raw
    .execute(
      "UPDATE connections SET api_provider = ?1",
      [r#"{"schema":1,"body":{"base_url":"https://api.deepseek.com","key":"00000000-0000-0000-0000-000000000000","models":[]}}"#],
    )
    .expect("damage");
  drop(raw);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  assert!(db.connections().is_err(), "a provider without models is not read as one");
}

#[test]
fn api_providers_a_database_from_before_them_runs_every_connection_on_its_sign_in() {
  let (repo, mut db) = project();
  let connection = claude(&repo);
  db.create_connection(&connection).expect("create");
  drop(db);
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    BEFORE_API_PROVIDERS,
  );
  let mut db = ProjectDb::open(old.path()).expect("migrated");
  let read = db.connection(connection.id()).expect("read").expect("kept");
  assert!(read.api_provider().is_none());
  let mut on_provider = AgentConnection::try_from(read).expect("an agent connection");
  on_provider
    .set_api_provider(Some(deepseek(SecretId::generate())))
    .expect("on Default");
  db.save_connection(&on_provider).expect("the new column takes it");
  assert!(
    db.connection(connection.id())
      .expect("read")
      .expect("kept")
      .api_provider()
      .is_some()
  );
}
