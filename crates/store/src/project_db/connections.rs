//! The `connections` table. No credentials are stored: the agent CLIs and the GitHub CLI keep
//! their own login, and a native agent connection names the project secret holding its key.

use std::path::PathBuf;

use orchester_core::{
  connection::{
    AgentConnection, AgentEndpoint, AgentProvider, ApiEndpoint, CheckResult, Connection, ConnectionCheck,
    ConnectionKind, GitHubCheck, GitHubCheckResult, GitHubHost, LastCheck, ModelRequirements,
  },
  id::ConnectionId,
  name::Name,
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use rusqlite::{Connection as Sqlite, OptionalExtension, Params, Row, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse, path_text},
};

const SELECT: &str = "SELECT id, kind, provider, name, settings, checked_at, last_check, created_at FROM connections";
const AGENT_PROVIDER: &str = "agent_provider";
const GITHUB: &str = "github";
/// The `provider` of a GitHub connection: the GitHub CLI is the only way to reach GitHub.
const GH_CLI: &str = "gh_cli";
const SETTINGS_SCHEMA: u32 = 1;
const CHECK_SCHEMA: u32 = 1;

/// Settings of an agent CLI connection.
#[derive(Serialize, Deserialize)]
struct ProviderSettings {
  executable: String,
  /// Absent before model requirements were recorded, and while none is.
  #[serde(default, skip_serializing_if = "ModelRequirements::is_empty")]
  model_requirements: ModelRequirements,
}

/// Settings of a native agent connection: its model API.
#[derive(Serialize, Deserialize)]
struct NativeSettings {
  api: ApiEndpoint,
}

/// Settings of a GitHub connection.
#[derive(Serialize, Deserialize)]
struct GitHubSettings {
  executable: String,
  host: GitHubHost,
}

impl ConnectionStore for ProjectDb {
  type Error = StoreError;

  fn create_connection(&mut self, connection: &Connection) -> Result<(), StoreError> {
    let columns = Columns::of(connection)?;
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, connection)?;
    transaction.execute(
      "INSERT INTO connections (id, kind, provider, name, settings, status, checked_at, last_check, created_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
      params![
        connection.id().to_string(),
        columns.kind,
        columns.provider,
        connection.name().as_str(),
        columns.settings,
        connection.status().as_str(),
        columns.checked_at,
        columns.last_check,
        connection.created_at().unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  fn save_connection(&mut self, connection: &Connection) -> Result<(), StoreError> {
    let columns = Columns::of(connection)?;
    let transaction = self.connection.transaction()?;
    ensure_name_free(&transaction, connection)?;
    let changed = transaction.execute(
      "UPDATE connections SET name = ?2, settings = ?3, status = ?4, checked_at = ?5, last_check = ?6 \
       WHERE id = ?1",
      params![
        connection.id().to_string(),
        connection.name().as_str(),
        columns.settings,
        connection.status().as_str(),
        columns.checked_at,
        columns.last_check
      ],
    )?;
    if changed == 0 {
      return Err(StoreError::ConnectionNotFound(connection.id()));
    }
    transaction.commit()?;
    Ok(())
  }

  fn record_model_requirement(&mut self, id: ConnectionId, model: &ModelId, required: &str) -> Result<(), StoreError> {
    // Immediate (the connection's behavior): no other writer changes the row between this read and
    // the write, and only the settings column, which holds the requirements, is written back.
    let transaction = self.connection.transaction()?;
    let stored = load(&transaction, "WHERE id = ?1", [id.to_string()])?
      .pop()
      .ok_or(StoreError::ConnectionNotFound(id))?;
    let mut connection = match AgentConnection::try_from(stored) {
      Ok(connection) if connection.endpoint().program().is_some() => connection,
      _ => return Err(StoreError::NotAnAgentCliConnection(id)),
    };
    connection.record_model_requirement(model, required);
    let settings = Columns::of(&connection)?.settings;
    transaction.execute(
      "UPDATE connections SET settings = ?2 WHERE id = ?1",
      params![id.to_string(), settings],
    )?;
    transaction.commit()?;
    Ok(())
  }

  fn connection(&self, id: ConnectionId) -> Result<Option<Connection>, StoreError> {
    Ok(load(&self.connection, "WHERE id = ?1", [id.to_string()])?.pop())
  }

  fn connections(&self) -> Result<Vec<Connection>, StoreError> {
    load(&self.connection, "ORDER BY name", [])
  }
}

/// Fails if another connection already has this connection's name.
fn ensure_name_free(sqlite: &Sqlite, connection: &Connection) -> Result<(), StoreError> {
  let taken: Option<String> = sqlite
    .query_row(
      "SELECT id FROM connections WHERE name = ?1 AND id <> ?2",
      params![connection.name().as_str(), connection.id().to_string()],
      |row| row.get(0),
    )
    .optional()?;
  match taken {
    Some(_) => Err(StoreError::DuplicateConnectionName(
      connection.name().as_str().to_owned(),
    )),
    None => Ok(()),
  }
}

/// Encoded columns of a connection.
struct Columns {
  kind: &'static str,
  provider: &'static str,
  settings: String,
  checked_at: Option<i64>,
  last_check: Option<String>,
}

impl Columns {
  fn of(connection: &Connection) -> Result<Self, StoreError> {
    let (kind, provider, settings) = match (connection.kind(), connection.endpoint()) {
      (ConnectionKind::Agent(provider), AgentEndpoint::Program(executable)) => {
        let settings = ProviderSettings {
          executable: path_text("connection executable", executable)?.to_owned(),
          model_requirements: connection.model_requirements().clone(),
        };
        (
          AGENT_PROVIDER,
          provider_text(*provider),
          encode_json("connection settings", SETTINGS_SCHEMA, &settings)?,
        )
      }
      (ConnectionKind::Agent(provider), AgentEndpoint::Api(api)) => {
        let settings = NativeSettings { api: api.clone() };
        (
          AGENT_PROVIDER,
          provider_text(*provider),
          encode_json("connection settings", SETTINGS_SCHEMA, &settings)?,
        )
      }
      (ConnectionKind::GitHub { host }, endpoint) => {
        let executable = endpoint
          .program()
          .expect("a GitHub connection is reached through its program");
        let settings = GitHubSettings {
          executable: path_text("connection executable", executable)?.to_owned(),
          host: host.clone(),
        };
        (
          GITHUB,
          GH_CLI,
          encode_json("connection settings", SETTINGS_SCHEMA, &settings)?,
        )
      }
    };
    let last_check = connection
      .last_check()
      .map(|check| match check {
        LastCheck::Agent(check) => encode_json("connection check", CHECK_SCHEMA, &check.result),
        LastCheck::GitHub(check) => encode_json("connection check", CHECK_SCHEMA, &check.result),
      })
      .transpose()?;
    Ok(Self {
      kind,
      provider,
      settings,
      checked_at: connection.last_check().map(|check| check.at().unix_millis()),
      last_check,
    })
  }
}

fn load(sqlite: &Sqlite, clause: &str, params: impl Params) -> Result<Vec<Connection>, StoreError> {
  let mut statement = sqlite.prepare(&format!("{SELECT} {clause}"))?;
  let rows = statement
    .query_map(params, StoredRow::read)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(StoredRow::into_connection).collect()
}

struct StoredRow {
  id: String,
  kind: String,
  provider: String,
  name: String,
  settings: String,
  checked_at: Option<i64>,
  last_check: Option<String>,
  created_at: i64,
}

impl StoredRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      kind: row.get(1)?,
      provider: row.get(2)?,
      name: row.get(3)?,
      settings: row.get(4)?,
      checked_at: row.get(5)?,
      last_check: row.get(6)?,
      created_at: row.get(7)?,
    })
  }

  fn into_connection(self) -> Result<Connection, StoreError> {
    let ((kind, endpoint, last_check), requirements) = match self.kind.as_str() {
      AGENT_PROVIDER => self.agent_parts()?,
      GITHUB => (self.github_parts()?, ModelRequirements::default()),
      other => return Err(corrupt("connection kind", other)),
    };
    Connection::from_stored(
      parse("connection id", &self.id)?,
      Name::new(&self.name).map_err(|error| corrupt("connection name", error))?,
      kind,
      endpoint,
      last_check,
      Timestamp::from_unix_millis(self.created_at),
    )
    .map(|connection| connection.with_model_requirements(requirements))
    .map_err(|error| corrupt("connection", error))
  }

  fn agent_parts(&self) -> Result<(Parts, ModelRequirements), StoreError> {
    let provider = parse_provider(&self.provider)?;
    let last_check = self
      .check::<CheckResult>()?
      .map(|(at, result)| LastCheck::Agent(ConnectionCheck { at, result }));
    let kind = ConnectionKind::Agent(provider);
    if provider.is_native() {
      let settings: NativeSettings = decode_json("connection settings", SETTINGS_SCHEMA, &self.settings)?;
      return Ok((
        (kind, AgentEndpoint::Api(settings.api), last_check),
        ModelRequirements::default(),
      ));
    }
    let settings: ProviderSettings = decode_json("connection settings", SETTINGS_SCHEMA, &self.settings)?;
    let endpoint = AgentEndpoint::Program(PathBuf::from(settings.executable));
    Ok(((kind, endpoint, last_check), settings.model_requirements))
  }

  fn github_parts(&self) -> Result<Parts, StoreError> {
    if self.provider != GH_CLI {
      return Err(corrupt("connection provider", &self.provider));
    }
    let settings: GitHubSettings = decode_json("connection settings", SETTINGS_SCHEMA, &self.settings)?;
    let last_check = self
      .check::<GitHubCheckResult>()?
      .map(|(at, result)| LastCheck::GitHub(GitHubCheck { at, result }));
    let kind = ConnectionKind::GitHub { host: settings.host };
    Ok((
      kind,
      AgentEndpoint::Program(PathBuf::from(settings.executable)),
      last_check,
    ))
  }

  fn check<T: DeserializeOwned>(&self) -> Result<Option<(Timestamp, T)>, StoreError> {
    match (self.checked_at, &self.last_check) {
      (Some(at), Some(result)) => Ok(Some((
        Timestamp::from_unix_millis(at),
        decode_json("connection check", CHECK_SCHEMA, result)?,
      ))),
      _ => Ok(None),
    }
  }
}

/// Kind, endpoint and last check of a stored connection.
type Parts = (ConnectionKind, AgentEndpoint, Option<LastCheck>);

const fn provider_text(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "codex_cli",
    AgentProvider::ClaudeCodeCli => "claude_code_cli",
    AgentProvider::AnthropicApi => "anthropic_api",
    AgentProvider::OpenAiApi => "openai_api",
  }
}

fn parse_provider(value: &str) -> Result<AgentProvider, StoreError> {
  match value {
    "codex_cli" => Ok(AgentProvider::CodexCli),
    "claude_code_cli" => Ok(AgentProvider::ClaudeCodeCli),
    "anthropic_api" => Ok(AgentProvider::AnthropicApi),
    "openai_api" => Ok(AgentProvider::OpenAiApi),
    other => Err(corrupt("connection provider", other)),
  }
}
