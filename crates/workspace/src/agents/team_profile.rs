//! What the panel of a team profile shows besides how it stands (`Agents · Profile` `NDZXC`, `WPvZx`, `QOBF2`): what
//! the team holds of it (its CLI kind, model, effort, the instructions, MCP servers, tools and skills it uses and the
//! secrets it needs, by name) and what this computer fills in (the connections of its CLI kind and the pick)
//! (`docs/design/team-configuration.md`, "What a computer fills in").

use orchester_core::{
  connection::{AgentProvider, Connection, ConnectionKind, ConnectionStatus, REASONING_EFFORT},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ResourceKind, TeamAgentProfile, TeamMcpServer},
  secret::SecretName,
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::readiness::team_profile_body;

/// A resource a profile uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
  pub id: ResourceId,
  /// Its name; its id while this computer has no resource of it (archived, or not synced yet), so nothing is hidden.
  pub name: String,
}

/// An MCP server a profile uses, with the secrets it takes by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsedServer {
  pub server: Listed,
  /// The secrets its variables and secret headers name; none while this computer has no version of it.
  pub secrets: Vec<SecretName>,
}

/// A connection of this computer that runs a profile's CLI kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionChoice {
  pub id: ConnectionId,
  pub name: String,
  /// The CLI's version its last check found.
  pub version: Option<String>,
  /// Its last check found the CLI signed out: a profile that runs on it is not ready until the person signs in.
  pub signed_out: bool,
}

/// One team profile as the panel shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamProfileSheet {
  pub cli: AgentProvider,
  pub model: String,
  pub effort: Option<String>,
  pub instructions: Vec<Listed>,
  pub mcp_servers: Vec<UsedServer>,
  pub tools: Vec<Listed>,
  pub skills: Vec<Listed>,
  /// The secrets it needs by name: those of the MCP servers it uses, then its own.
  pub secrets: Vec<SecretName>,
  /// The connections of this computer of its CLI kind.
  pub connections: Vec<ConnectionChoice>,
  /// The connection picked for it, while it is still one of [`Self::connections`].
  pub picked: Option<ConnectionId>,
}

impl TeamProfileSheet {
  /// The connection it runs on here: the pick, else the only one of its kind; `None` when none fits or the person has
  /// to pick among several.
  pub fn runs_on(&self) -> Option<&ConnectionChoice> {
    let found = |id| self.connections.iter().find(|choice| choice.id == id);
    match (self.picked.and_then(found), self.connections.as_slice()) {
      (Some(picked), _) => Some(picked),
      (None, [only]) => Some(only),
      (None, _) => None,
    }
  }
}

/// A sheet for each team profile whose newest version is in the team's form; one that is not has none, and its
/// readiness says why.
pub(super) fn sheets(db: &ProjectDb) -> Result<Vec<(ResourceId, TeamProfileSheet)>, StoreError> {
  let waiting = db.waiting_publishes()?;
  let connections = db.connections()?;
  let mut sheets = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let Ok(team) = team_profile_body(db, resource.id(), &version, &waiting)? else {
      continue;
    };
    sheets.push((resource.id(), sheet(db, resource.id(), &team, &connections)?));
  }
  Ok(sheets)
}

fn sheet(
  db: &ProjectDb,
  profile: ResourceId,
  team: &TeamAgentProfile,
  connections: &[Connection],
) -> Result<TeamProfileSheet, StoreError> {
  let named = |ids: &[ResourceId]| ids.iter().map(|id| listed(db, *id)).collect::<Result<Vec<_>, _>>();
  let mut mcp_servers = Vec::new();
  for server in team.mcp_servers() {
    let secrets = match db.latest_version::<TeamMcpServer>(*server)? {
      Some(found) => found.body().secrets(),
      None => Vec::new(),
    };
    mcp_servers.push(UsedServer {
      server: listed(db, *server)?,
      secrets,
    });
  }
  let mut secrets: Vec<SecretName> = Vec::new();
  let own = team.secrets().iter().cloned();
  for name in mcp_servers
    .iter()
    .flat_map(|used| used.secrets.iter().cloned())
    .chain(own)
  {
    if !secrets.contains(&name) {
      secrets.push(name);
    }
  }
  let of_kind: Vec<ConnectionChoice> = connections
    .iter()
    .filter(|connection| matches!(connection.kind(), ConnectionKind::Agent(provider) if *provider == team.cli()))
    .map(|connection| ConnectionChoice {
      id: connection.id(),
      name: connection.name().to_string(),
      version: connection.version().map(str::to_owned),
      signed_out: connection.status() == ConnectionStatus::SignInRequired,
    })
    .collect();
  let picked = db
    .connection_pick(profile)?
    .map(|pick| pick.connection)
    .filter(|pick| of_kind.iter().any(|choice| choice.id == *pick));
  Ok(TeamProfileSheet {
    cli: team.cli(),
    model: team.model().as_str().to_owned(),
    effort: team
      .settings()
      .iter()
      .find_map(|(key, value)| (key == REASONING_EFFORT).then(|| value.to_owned())),
    instructions: named(team.instruction_resources())?,
    mcp_servers,
    tools: named(team.tools())?,
    skills: named(team.skills())?,
    secrets,
    connections: of_kind,
    picked,
  })
}

fn listed(db: &ProjectDb, id: ResourceId) -> Result<Listed, StoreError> {
  let name = db
    .resource(id)?
    .map_or_else(|| id.to_string(), |found| found.name().to_string());
  Ok(Listed { id, name })
}
