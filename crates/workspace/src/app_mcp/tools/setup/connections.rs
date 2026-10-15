//! `list_connections`, `add_cli_connection`, `check_connection` and `list_models`: a project's
//! connections as Connections lists them, a Codex or Claude CLI added as Add connection adds it (checked
//! first, saved only when it answers as the expected CLI), checked again, and the models the profile
//! editor offers for one. No secret, nor its value, is listed.

use std::path::PathBuf;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, CheckResult, Connection, ConnectionCheck, ConnectionError},
  id::{ConnectionId, ProjectId},
  name::Name,
  secret::Secret,
  store::ConnectionStore,
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Translate,
    across::{InProjects, across},
    wire,
  },
  widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::{
    data::{ConnectionDraft, ProjectConnections, SaveError, recheck_in, save_in},
    probe,
  },
  links::Link,
  messages::{self, Message},
  tasks::outside::Outside,
};

/// The agent CLIs a connection can be added for.
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum CliKind {
  Codex,
  Claude,
}

impl CliKind {
  const fn provider(self) -> AgentProvider {
    match self {
      Self::Codex => AgentProvider::CodexCli,
      Self::Claude => AgentProvider::ClaudeCodeCli,
    }
  }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct AddCliConnection {
  /// The project's id, or its name.
  pub project: String,
  pub kind: CliKind,
  /// The connection's name, unique among the project's connections, e.g. "Codex · Kontur".
  pub name: String,
  /// The CLI's absolute path; left out, the newest copy on the search path.
  pub program: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InConnection {
  /// The project's id, or its name.
  pub project: String,
  /// The connection's id, or its name.
  pub connection: String,
}

pub async fn list_connections(
  services: &Services,
  args: InProjects,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || {
    across(&services, args.project.as_deref(), |_, db| {
      Ok(ProjectConnections::load(db)?)
    })
  })
  .await?;
  let agents = read.items(|open, listed| {
    listed
      .connections
      .iter()
      .map(|connection| agent_json(connection, &listed.secrets, open.project.id()))
      .collect()
  });
  let github = read.items(|open, listed| {
    listed
      .github
      .iter()
      .map(|connection| {
        json!({
          "id": connection.id().to_string(),
          "name": connection.name().as_str(),
          "host": connection.host().to_string(),
          "status": connection.status().as_str(),
          "link": Link::Connection(open.project.id(), connection.id()).path(),
        })
      })
      .collect()
  });
  let mcp = read.items(|open, listed| super::mcp_servers::servers_json(listed, open.project.id()));
  Ok(json!({
    "agents": agents,
    "github": github,
    "mcp_servers": mcp,
    "unreadable_projects": read.unreadable(translate),
  }))
}

pub async fn add_cli_connection(services: &Services, args: AddCliConnection) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let provider = args.kind.provider();
    let name = Name::new(&args.name).map_err(|error| refused("invalid_name", messages::name_error(&error)))?;
    let found = probe::detect_agent(provider);
    let program = match args.program {
      Some(program) => PathBuf::from(program),
      None => found.first().cloned().ok_or_else(|| {
        let program = provider.program_name().unwrap_or_default();
        refused(
          "program_not_found",
          Message::new("appMcp.errors.cliNotFound").with("program", program.to_owned()),
        )
      })?,
    };
    if !program.is_absolute() {
      return Err(refused(
        "program_not_absolute",
        Message::new("errors.executableNotAbsolute"),
      ));
    }
    // A program of automation's own choosing would run as the project's agent: a person's decision,
    // made before anything runs it. A copy found on the search path is saved as the search path names it,
    // never as the client named it: a link the client chose could be pointed elsewhere later.
    let program = match probe::found_copy(&found, &program) {
      Some(copy) => copy.to_path_buf(),
      None => {
        let reason = Message::new("appMcp.widens.program").with("program", program.display().to_string());
        widening::allowed(&services, vec![reason])?;
        program
      }
    };
    let check = probe::check(provider, &program).expect("an agent CLI is checked by running it");
    let draft = ConnectionDraft {
      id: None,
      name,
      provider,
      executable: program,
      check: check.clone(),
    };
    let id = save_in(&open.repo_root, draft).map_err(|error| save_error(&error, &check))?;
    services.changes.record(open.project.id(), Outside::Connections);
    let listed = ProjectConnections::load_in(&open.repo_root)?;
    let connection = listed
      .connection(id)
      .ok_or_else(|| ToolError::not_found(Missing::Connection, id.to_string()))?;
    Ok(json!({
      "connection": agent_json(connection, &listed.secrets, open.project.id()),
      "check": check_json(&check),
    }))
  })
  .await
}

pub async fn check_connection(services: &Services, args: InConnection) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let id = connection_named(&ProjectDb::open(&open.repo_root)?, &args.connection)?.id();
    recheck_in(&open.repo_root, id).map_err(|error| match error {
      SaveError::NativeNotAvailable => refused("not_checked", error.message()),
      other => ToolError::refused("project_data", vec![other.message()]),
    })?;
    services.changes.record(open.project.id(), Outside::Connections);
    let listed = ProjectConnections::load_in(&open.repo_root)?;
    let connection = match (listed.connection(id), listed.github_connection(id)) {
      (Some(agent), _) => agent_json(agent, &listed.secrets, open.project.id()),
      (None, Some(github)) => json!({ "id": id.to_string(), "status": github.status().as_str() }),
      (None, None) => return Err(ToolError::not_found(Missing::Connection, &args.connection)),
    };
    Ok(json!({ "connection": connection }))
  })
  .await
}

pub async fn list_models(services: &Services, args: InConnection) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let connection = agent_named(&ProjectDb::open(&open.repo_root)?, &args.connection)?;
    let models: Vec<Value> = connection
      .models()
      .iter()
      .map(|model| json!({ "id": model.id().as_str(), "efforts": model.efforts() }))
      .collect();
    Ok(json!({
      "connection": connection.name().as_str(),
      "checked": connection.last_check().is_some(),
      "source": connection.catalog().map(|catalog| catalog.source()),
      "models": models,
      "link": Link::Connection(open.project.id(), connection.id()).path(),
    }))
  })
  .await
}

/// The project's connection named by `named`: its id or, ignoring case, its name.
pub(super) fn connection_named(db: &ProjectDb, named: &str) -> Result<Connection, ToolError> {
  let lowered = named.to_lowercase();
  db.connections()?
    .into_iter()
    .find(|connection| connection.id().to_string() == named || connection.name().as_str().to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Connection, named))
}

/// The project's agent connection named by `named`, as [`connection_named`] finds it.
pub(super) fn agent_named(db: &ProjectDb, named: &str) -> Result<AgentConnection, ToolError> {
  AgentConnection::try_from(connection_named(db, named)?).map_err(|_| {
    refused(
      "not_an_agent_connection",
      Message::new("appMcp.errors.notAnAgentConnection").with("connection", named.to_owned()),
    )
  })
}

/// The agent connection `id` among `connections`, by its id.
pub(super) fn agent_by_id(connections: &[AgentConnection], id: ConnectionId) -> Option<&AgentConnection> {
  connections.iter().find(|connection| connection.id() == id)
}

/// An agent connection of `project` as Connections lists it, with its link: its inspector.
fn agent_json(connection: &AgentConnection, secrets: &[Secret], project: ProjectId) -> Value {
  let check = connection.last_check();
  let version = check.and_then(|check| match &check.result {
    CheckResult::Available { version, .. } => Some(version.clone()),
    _ => None,
  });
  json!({
    "id": connection.id().to_string(),
    "name": connection.name().as_str(),
    "provider": connection.provider(),
    "program": connection.executable().map(|program| program.display().to_string()),
    "status": connection.status_with(secrets).as_str(),
    "version": version,
    "checked_at": check.map(|check| wire::time(check.at)),
    "models": connection.models().len(),
    "account": connection.account().id(connection.provider()).map(|account| account.to_string()),
    "link": Link::Connection(project, connection.id()).path(),
  })
}

fn check_json(check: &ConnectionCheck) -> Value {
  let (version, models) = match &check.result {
    CheckResult::Available { version, catalog, .. } => (
      Some(version.clone()),
      catalog
        .models()
        .iter()
        .map(|model| model.id().as_str().to_owned())
        .collect(),
    ),
    _ => (None, Vec::new()),
  };
  json!({
    "status": check.result.status().as_str(),
    "verified": check.result.is_verified(),
    "version": version,
    "models": models,
  })
}

/// Why the connection was not saved, with what its check found when the CLI did not answer as expected.
fn save_error(error: &SaveError, check: &ConnectionCheck) -> ToolError {
  let code = match error {
    SaveError::NotVerified => {
      let found = Message::new("appMcp.errors.checkFound").with("status", check.result.status().as_str().to_owned());
      return ToolError::refused("not_verified", vec![error.message(), found]);
    }
    SaveError::Store(StoreError::DuplicateConnectionName(_)) => "name_taken",
    SaveError::Connection(ConnectionError::RelativeExecutable(_)) => "program_not_absolute",
    SaveError::NativeNotAvailable => "not_checked",
    SaveError::Store(_) | SaveError::Connection(_) => "project_data",
  };
  ToolError::refused(code, vec![error.message()])
}

fn refused(code: &str, reason: Message) -> ToolError {
  ToolError::refused(code, vec![reason])
}
