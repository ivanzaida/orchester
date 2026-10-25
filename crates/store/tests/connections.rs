use std::{fs, path::PathBuf};

use orchester_core::{
  connection::{
    AgentConnection, AgentEndpoint, AgentProvider, ApiEndpoint, BaseUrl, CatalogModel, CatalogSource, CheckResult,
    ConnectionCheck, ConnectionStatus, Effort, EndpointPreset, GitHubCheck, GitHubCheckResult, GitHubConnection,
    GitHubHost, GitHubLogin, LastCheck, ModelCatalog, ModelSource, NativeModel, NativeModels, SignIn,
  },
  id::{ConnectionId, ProjectId, SecretId},
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

fn connection(repo: &TempDir, name: &str, provider: AgentProvider) -> AgentConnection {
  AgentConnection::new(
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
  assert_eq!(db.connection(codex.id()).expect("read").as_ref(), Some(&*codex));

  codex
    .record_check(available())
    .expect("an agent check fits a CLI connection");
  codex.rename(Name::new("Codex · Kontur").expect("valid name"));
  db.save_connection(&codex).expect("save");
  let stored = db.connection(codex.id()).expect("read").expect("exists");
  let stored = AgentConnection::try_from(stored).expect("an agent connection");
  assert_eq!(stored, codex);
  assert_eq!(stored.status(), ConnectionStatus::Connected);
  assert_eq!(stored.models()[0].efforts(), ["low", "high"]);

  let reopened = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(reopened.connections().expect("list"), [codex.into_connection()]);
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
fn renaming_and_recording_a_check_each_keep_what_the_other_stored() {
  let (repo, mut db) = project_repo();
  let codex = connection(&repo, "Codex", AgentProvider::CodexCli);
  let claude = connection(&repo, "Claude", AgentProvider::ClaudeCodeCli);
  db.create_connection(&codex).expect("create codex");
  db.create_connection(&claude).expect("create claude");

  db.rename_connection(codex.id(), &Name::new("Codex · Kontur").expect("valid name"))
    .expect("renamed");
  db.record_connection_check(codex.id(), LastCheck::Agent(available()))
    .expect("checked");
  let stored = db.connection(codex.id()).expect("read").expect("exists");
  assert_eq!(stored.name().as_str(), "Codex · Kontur", "the check keeps the name");
  assert_eq!(stored.status(), ConnectionStatus::Connected);
  assert_eq!(stored.version(), Some("0.139.0"));

  db.rename_connection(codex.id(), &Name::new("Codex · main").expect("valid name"))
    .expect("renamed again");
  let stored = db.connection(codex.id()).expect("read").expect("exists");
  assert_eq!(
    stored.status(),
    ConnectionStatus::Connected,
    "the rename keeps the check"
  );

  assert!(matches!(
    db.rename_connection(codex.id(), claude.name()),
    Err(StoreError::DuplicateConnectionName(name)) if name == "Claude"
  ));
  let gone = ConnectionId::generate();
  assert!(matches!(
    db.rename_connection(gone, &Name::new("Gone").expect("valid name")),
    Err(StoreError::ConnectionNotFound(id)) if id == gone
  ));
  assert!(matches!(
    db.record_connection_check(gone, LastCheck::Agent(available())),
    Err(StoreError::ConnectionNotFound(id)) if id == gone
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
  codex
    .record_check(available())
    .expect("an agent check fits a CLI connection");
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

fn github(repo: &TempDir) -> GitHubConnection {
  GitHubConnection::new(
    ConnectionId::generate(),
    Name::new("GitHub").expect("valid name"),
    GitHubHost::github_com(),
    executable(repo, "gh.exe"),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("absolute executable")
}

#[test]
fn github_connection_round_trip_keeps_host_and_check() {
  let (repo, mut db) = project_repo();
  let mut github = github(&repo);
  db.create_connection(&github).expect("create");
  github.record_check(GitHubCheck {
    at: Timestamp::from_unix_millis(2_000),
    result: GitHubCheckResult::Available {
      version: "2.97.0".into(),
      account: Some(GitHubLogin::new("ivanzaida").expect("valid login")),
      scopes: vec!["repo".into(), "workflow".into()],
      sign_in: SignIn::SignedIn,
    },
  });
  db.save_connection(&github).expect("save");

  let reopened = ProjectDb::open(repo.path()).expect("reopen");
  let stored = reopened.connection(github.id()).expect("read").expect("exists");
  let stored = GitHubConnection::try_from(stored).expect("a GitHub connection");
  assert_eq!(stored, github);
  assert_eq!(stored.host().as_str(), "github.com");
  assert_eq!(stored.status(), ConnectionStatus::Connected);
  assert!(AgentConnection::try_from(stored.into_connection()).is_err());
}

#[test]
fn github_connection_rows_hold_executable_and_host_only() {
  let (repo, mut db) = project_repo();
  db.create_connection(&github(&repo)).expect("create");
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let (kind, provider, status, settings): (String, String, String, String) = raw
    .query_row("SELECT kind, provider, status, settings FROM connections", [], |row| {
      Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .expect("row");
  assert_eq!(
    (kind.as_str(), provider.as_str(), status.as_str()),
    ("github", "gh_cli", "not_checked")
  );
  let settings: serde_json::Value = serde_json::from_str(&settings).expect("JSON settings");
  assert_eq!(settings["body"]["host"], "github.com");
  let mut keys: Vec<&String> = settings["body"].as_object().expect("settings body").keys().collect();
  keys.sort();
  assert_eq!(keys, ["executable", "host"]);
}

#[test]
fn damaged_github_host_is_reported() {
  let (repo, mut db) = project_repo();
  db.create_connection(&github(&repo)).expect("create");
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  raw
    .execute(
      "UPDATE connections SET settings = json_set(settings, '$.body.host', 'https://github.com')",
      [],
    )
    .expect("damage the row");
  assert!(matches!(
    db.connections(),
    Err(StoreError::Corrupt {
      what: "connection settings",
      ..
    })
  ));
}

#[test]
fn connection_round_trip_keeps_the_model_requirements_a_refusal_recorded() {
  let (repo, mut db) = project_repo();
  let mut claude = connection(&repo, "Claude", AgentProvider::ClaudeCodeCli);
  db.create_connection(&claude).expect("create");
  let settings = || -> String {
    Sqlite::open(repo.path().join(".orchester").join("orchester.db"))
      .expect("open the database")
      .query_row("SELECT settings FROM connections", [], |row| row.get(0))
      .expect("settings")
  };
  assert!(
    !settings().contains("model_requirements"),
    "left out while none is recorded"
  );
  let opus = ModelId::new("claude-opus-5-5").expect("valid model");
  claude.record_model_requirement(&opus, "2.1.280");
  db.save_connection(&claude).expect("save");
  let reopened = ProjectDb::open(repo.path()).expect("reopen");
  let stored = AgentConnection::try_from(reopened.connection(claude.id()).expect("read").expect("exists"))
    .expect("an agent connection");
  assert_eq!(stored.model_requirements().required(&opus), Some("2.1.280"));
  assert!(settings().contains(r#""model_requirements":{"claude-opus-5-5":"2.1.280"}"#));
}

#[test]
fn recording_a_model_requirement_keeps_an_edit_saved_since_the_runner_read_the_connection() {
  let (repo, mut runner) = project_repo();
  let claude = connection(&repo, "Claude", AgentProvider::ClaudeCodeCli);
  runner.create_connection(&claude).expect("create");
  let read_by_runner = runner.connection(claude.id()).expect("read").expect("exists");
  let mut desktop = ProjectDb::open(repo.path()).expect("open as the desktop");
  let mut edited = claude.clone();
  edited.rename(Name::new("Claude · Kontur").expect("valid name"));
  edited
    .set_executable(executable(&repo, "claude.exe"))
    .expect("absolute executable");
  edited
    .record_check(available())
    .expect("an agent check fits a CLI connection");
  desktop.save_connection(&edited).expect("the desktop saves its edit");

  let opus = ModelId::new("claude-opus-5-5").expect("valid model");
  runner
    .record_model_requirement(read_by_runner.id(), &opus, "2.1.280")
    .expect("recorded");

  let stored = AgentConnection::try_from(desktop.connection(claude.id()).expect("read").expect("exists"))
    .expect("an agent connection");
  assert_eq!(stored.model_requirements().required(&opus), Some("2.1.280"));
  let mut expected = edited;
  expected.record_model_requirement(&opus, "2.1.280");
  assert_eq!(stored, expected, "name, executable and models of the edit survive");
}

#[test]
fn saving_a_copy_read_before_the_runner_recorded_a_model_requirement_keeps_it() {
  let (repo, mut desktop) = project_repo();
  let claude = connection(&repo, "Claude", AgentProvider::ClaudeCodeCli);
  desktop.create_connection(&claude).expect("create");
  let mut stale = AgentConnection::try_from(desktop.connection(claude.id()).expect("read").expect("exists"))
    .expect("an agent connection");
  let mut runner = ProjectDb::open(repo.path()).expect("open as the runner");
  let opus = ModelId::new("claude-opus-5-5").expect("valid model");
  runner
    .record_model_requirement(claude.id(), &opus, "2.1.280")
    .expect("recorded");

  stale.rename(Name::new("Claude · Kontur").expect("valid name"));
  desktop
    .save_connection(&stale)
    .expect("the desktop saves its stale copy");

  let stored = AgentConnection::try_from(desktop.connection(claude.id()).expect("read").expect("exists"))
    .expect("an agent connection");
  assert_eq!(stored.name().as_str(), "Claude · Kontur");
  assert_eq!(stored.model_requirements().required(&opus), Some("2.1.280"));
}

#[test]
fn recording_a_model_requirement_on_a_removed_connection_fails_and_creates_nothing() {
  let (repo, mut db) = project_repo();
  let claude = connection(&repo, "Claude", AgentProvider::ClaudeCodeCli);
  db.create_connection(&claude).expect("create");
  Sqlite::open(repo.path().join(".orchester").join("orchester.db"))
    .expect("open the database")
    .execute("DELETE FROM connections WHERE id = ?1", [claude.id().to_string()])
    .expect("the desktop removes the connection");

  let opus = ModelId::new("claude-opus-5-5").expect("valid model");
  assert!(matches!(
    db.record_model_requirement(claude.id(), &opus, "2.1.280"),
    Err(StoreError::ConnectionNotFound(id)) if id == claude.id()
  ));
  assert!(db.connections().expect("list").is_empty());
}

#[test]
fn recording_a_model_requirement_on_a_native_connection_fails_and_changes_nothing() {
  let (_repo, mut db) = project_repo();
  let api = native(
    AgentProvider::AnthropicApi,
    EndpointPreset::Official,
    "https://api.anthropic.com",
    SecretId::generate(),
  );
  db.create_connection(&api).expect("create");

  let opus = ModelId::new("claude-opus-5-5").expect("valid model");
  assert!(matches!(
    db.record_model_requirement(api.id(), &opus, "2.1.280"),
    Err(StoreError::NotAnAgentCliConnection(id)) if id == api.id()
  ));
  assert_eq!(db.connection(api.id()).expect("read").as_ref(), Some(&*api));
}

fn native(provider: AgentProvider, preset: EndpointPreset, url: &str, key: SecretId) -> AgentConnection {
  AgentConnection::native(
    ConnectionId::generate(),
    Name::new(&format!("{url} · Kontur")).expect("valid name"),
    provider,
    ApiEndpoint::new(preset, BaseUrl::new(url).expect("valid base URL"), key),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("native connection")
}

#[test]
fn native_connection_round_trip_keeps_provider_and_endpoint() {
  let (repo, mut db) = project_repo();
  let key = SecretId::generate();
  let claude = native(
    AgentProvider::AnthropicApi,
    EndpointPreset::Official,
    "https://api.anthropic.com",
    key,
  );
  let deepseek = native(
    AgentProvider::OpenAiApi,
    EndpointPreset::DeepSeek,
    "https://api.deepseek.com",
    SecretId::generate(),
  );
  db.create_connection(&claude).expect("create");
  db.create_connection(&deepseek).expect("create");

  let reopened = ProjectDb::open(repo.path()).expect("reopen");
  for connection in [&claude, &deepseek] {
    let stored = reopened.connection(connection.id()).expect("read").expect("exists");
    let stored = AgentConnection::try_from(stored).expect("an agent connection");
    assert_eq!(&stored, connection);
    assert_eq!(stored.status(), ConnectionStatus::NotChecked);
    assert_eq!(stored.executable(), None);
  }
  let stored = AgentConnection::try_from(reopened.connection(claude.id()).expect("read").expect("exists"))
    .expect("an agent connection");
  assert_eq!(stored.provider(), AgentProvider::AnthropicApi);
  let AgentEndpoint::Api(api) = stored.endpoint() else {
    panic!("a native connection is reached at its API");
  };
  assert_eq!(
    (api.preset(), api.base_url().as_str(), api.key()),
    (EndpointPreset::Official, "https://api.anthropic.com", key)
  );
}

#[test]
fn native_connection_rows_hold_the_endpoint_and_the_secret_id_only() {
  let (repo, mut db) = project_repo();
  let key = SecretId::generate();
  let deepseek = native(
    AgentProvider::OpenAiApi,
    EndpointPreset::DeepSeek,
    "https://api.deepseek.com/",
    key,
  );
  db.create_connection(&deepseek).expect("create");
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let (kind, provider, status, settings): (String, String, String, String) = raw
    .query_row("SELECT kind, provider, status, settings FROM connections", [], |row| {
      Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .expect("row");
  assert_eq!(
    (kind.as_str(), provider.as_str(), status.as_str()),
    ("agent_provider", "openai_api", "not_checked")
  );
  let settings: serde_json::Value = serde_json::from_str(&settings).expect("JSON settings");
  assert_eq!(
    settings["body"],
    serde_json::json!({
      "api": {"preset": "deepseek", "base_url": "https://api.deepseek.com", "key": key.to_string()}
    })
  );
}

#[test]
fn native_connection_round_trip_keeps_models_fallback_and_api_check() {
  let (repo, mut db) = project_repo();
  let opus = ModelId::new("claude-opus-5-5").expect("model");
  let fetched = ModelSource::Endpoint {
    at: Timestamp::from_unix_millis(2_000),
  };
  let models = NativeModels::new(
    vec![
      NativeModel::new(
        opus.clone(),
        std::num::NonZeroU32::new(200_000),
        std::num::NonZeroU32::new(64_000),
        vec![Effort::Medium, Effort::Max],
        fetched,
      ),
      NativeModel::new(
        ModelId::new("lab-mini-8b").expect("model"),
        None,
        None,
        Vec::new(),
        ModelSource::Manual,
      ),
    ],
    Some(opus),
    Some(Effort::Max),
  )
  .expect("models");
  let endpoint = ApiEndpoint::new(
    EndpointPreset::Official,
    BaseUrl::new("https://api.anthropic.com").expect("valid base URL"),
    SecretId::generate(),
  )
  .with_models(models)
  .with_provider_fallback(true);
  let mut claude = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Claude API · Kontur").expect("valid name"),
    AgentProvider::AnthropicApi,
    endpoint,
    Timestamp::from_unix_millis(1_000),
  )
  .expect("native connection");
  let check = ConnectionCheck {
    at: Timestamp::from_unix_millis(3_000),
    result: CheckResult::BillingProblem {
      balance: Some("0.00".into()),
    },
  };
  claude.record_check(check).expect("an API check");
  db.create_connection(&claude).expect("create");
  let reopened = ProjectDb::open(repo.path()).expect("reopen");
  let stored = AgentConnection::try_from(reopened.connection(claude.id()).expect("read").expect("exists"))
    .expect("an agent connection");
  assert_eq!(stored, claude);
  assert_eq!(stored.status(), ConnectionStatus::BillingProblem);
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let status: String = raw
    .query_row("SELECT status FROM connections", [], |row| row.get(0))
    .expect("row");
  assert_eq!(status, "billing_problem");
}

#[test]
fn damaged_native_base_url_is_reported() {
  let (repo, mut db) = project_repo();
  let claude = native(
    AgentProvider::AnthropicApi,
    EndpointPreset::Official,
    "https://api.anthropic.com",
    SecretId::generate(),
  );
  db.create_connection(&claude).expect("create");
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  raw
    .execute(
      "UPDATE connections SET settings = json_set(settings, '$.body.api.base_url', 'http://api.anthropic.com')",
      [],
    )
    .expect("damage the row");
  assert!(matches!(
    db.connections(),
    Err(StoreError::Corrupt {
      what: "connection settings",
      ..
    })
  ));
}
