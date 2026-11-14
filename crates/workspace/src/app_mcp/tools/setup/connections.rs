//! `list_connections` and `check_connection`: what a project is connected to. Its agents run on the person's
//! accounts (`docs/design/agent-accounts.md`, "Agents run on accounts"), so no agent connection is listed, named or
//! taken: `runs_on` says what each profile runs on on this computer, by account ([`super::runs_on`]). GitHub, the
//! MCP servers and the decision models are the project's own and are listed as before. No secret's value is listed.

use orchester_core::{
  connection::{AgentConnection, Connection},
  id::ConnectionId,
  store::ConnectionStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Translate,
    across::{InProjects, across},
  },
  profiles::{connection_here, listed_profiles},
  runs_on::Accounts,
};
use crate::{
  agents::data::ProjectAgents,
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  connections::data::{ProjectConnections, recheck_in},
  links::Link,
  messages::Message,
  tasks::outside::Outside,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InConnection {
  /// The project's id, or its name.
  pub project: String,
  /// The GitHub connection's id, or its name.
  pub connection: String,
}

/// What a project is connected to, as read for `list_connections`.
struct Listed {
  connections: ProjectConnections,
  /// What each listed profile runs on here.
  runs_on: Vec<Value>,
}

pub async fn list_connections(
  services: &Services,
  args: InProjects,
  translate: &Translate,
) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || {
    let accounts = Accounts::load(&services)?;
    across(&services, args.project.as_deref(), |_, db| {
      let connections = ProjectConnections::load(db)?.with_api_accounts(&accounts.api);
      let runs_on = runs_on(db, &connections, &accounts)?;
      Ok(Listed { connections, runs_on })
    })
  })
  .await?;
  let runs_on = read.items(|_, listed| listed.runs_on.clone());
  let github = read.items(|open, listed| {
    listed
      .connections
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
  let mcp = read.items(|open, listed| super::mcp_servers::servers_json(&listed.connections, open.project.id()));
  let decisions = read.items(|_, listed| {
    listed
      .connections
      .decisions
      .iter()
      .map(super::decision_connections::connection_json)
      .collect()
  });
  Ok(json!({
    "runs_on": runs_on,
    "github": github,
    "mcp_servers": mcp,
    "decision_models": decisions,
    "unreadable_projects": read.unreadable(translate),
  }))
}

/// What each profile the project lists runs on on this computer: the account or API provider, and whether a run
/// can start on it now; `runs_on` is null for a profile that runs on nothing here yet.
fn runs_on(db: &ProjectDb, connections: &ProjectConnections, accounts: &Accounts) -> Result<Vec<Value>, ToolError> {
  let agents = ProjectAgents::load(db)?;
  let mut rows = Vec::new();
  for (id, _) in listed_profiles(db, &agents)? {
    let Some(entry) = agents.profile(id) else {
      continue;
    };
    let here = connection_here(db, &agents, entry)?;
    let picked = db.connection_pick(id)?.and_then(|pick| pick.account);
    rows.push(json!({
      "profile": { "id": id.to_string(), "role": entry.profile.role().as_str() },
      "model": entry.profile.model().as_str(),
      "runs_on": here.map(|connection| accounts.runs_on_json(connection, picked)),
      "status": here.map(|connection| connections.status_of(connection).as_str()),
    }));
  }
  Ok(rows)
}

/// Checks the project's GitHub connection again, as *Check again* beside the repository does: `gh` is run here.
/// What an agent runs on is an account's, checked by `check_account`.
pub async fn check_connection(services: &Services, args: InConnection) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let connection = connection_named(&ProjectDb::open(&open.repo_root)?, &args.connection)?;
    let id = connection.id();
    if AgentConnection::try_from(connection).is_ok() {
      return Err(ToolError::refused(
        "not_a_github_connection",
        vec![Message::new("appMcp.errors.checkAccountInstead")],
      ));
    }
    recheck_in(&open.repo_root, id).map_err(|error| ToolError::refused("project_data", vec![error.message()]))?;
    services.changes.record(open.project.id(), Outside::Connections);
    let listed = ProjectConnections::load_in(&open.repo_root)?;
    let github = listed
      .github_connection(id)
      .ok_or_else(|| ToolError::not_found(Missing::Connection, &args.connection))?;
    Ok(json!({ "connection": { "id": id.to_string(), "status": github.status().as_str() } }))
  })
  .await
}

/// The project's connection named by `named`: its id or, ignoring case, its name.
fn connection_named(db: &ProjectDb, named: &str) -> Result<Connection, ToolError> {
  let lowered = named.to_lowercase();
  db.connections()?
    .into_iter()
    .find(|connection| connection.id().to_string() == named || connection.name().as_str().to_lowercase() == lowered)
    .ok_or_else(|| ToolError::not_found(Missing::Connection, named))
}

/// The agent connection `id` among `connections`, by its id.
pub(super) fn agent_by_id(connections: &[AgentConnection], id: ConnectionId) -> Option<&AgentConnection> {
  connections.iter().find(|connection| connection.id() == id)
}
