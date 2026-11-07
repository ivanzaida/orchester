//! What each profile, MCP server and tool of the project asks of this computer, from the version it uses.

use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, Resource, ResourceKind, TeamAgentProfile, TeamMcpServer, Tool, VersionNumber},
  sharing::{McpNeed, Needs, NeedsError},
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::standing::Undecided;

/// A resource and what it needs, before it is checked against what this computer has.
pub(super) struct Entry {
  pub(super) resource: ResourceId,
  pub(super) kind: ResourceKind,
  pub(super) name: String,
  pub(super) number: VersionNumber,
  pub(super) needs: Result<Needs, Undecided>,
  pub(super) off_here: bool,
}

impl Entry {
  fn new(resource: &Resource, number: VersionNumber, needs: Result<Needs, Undecided>) -> Self {
    Self {
      resource: resource.id(),
      kind: resource.kind(),
      name: resource.name().as_str().to_owned(),
      number,
      needs,
      off_here: false,
    }
  }
}

/// A team MCP server the project offers, with the version it is read from.
pub(super) struct Server {
  pub(super) entry: Entry,
  pub(super) need: McpNeed,
}

/// A profile's entry and the connection it runs on: the pick, or its own connection in a local project.
pub(super) struct Profile {
  pub(super) entry: Entry,
  pub(super) pick: Option<ConnectionId>,
}

pub(super) fn servers(db: &ProjectDb) -> Result<Vec<Server>, StoreError> {
  let mut servers = Vec::new();
  for resource in db.resources(ResourceKind::McpServer)? {
    let Some(version) = db.latest_version::<TeamMcpServer>(resource.id())? else {
      continue;
    };
    let need = McpNeed::of(resource.id(), version.body());
    let needs = Needs::mcp_server(&need);
    servers.push(Server {
      entry: Entry::new(&resource, version.number(), Ok(needs)),
      need,
    });
  }
  Ok(servers)
}

pub(super) fn profiles(db: &ProjectDb, team: bool, servers: &[McpNeed]) -> Result<Vec<Profile>, StoreError> {
  let mut profiles = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let (needs, pick) = if team {
      (
        team_profile(db, resource.id(), version.number(), servers)?,
        db.connection_pick(resource.id())?.map(|pick| pick.connection),
      )
    } else {
      local_profile(db, resource.id(), version.body())?
    };
    profiles.push(Profile {
      entry: Entry::new(&resource, version.number(), needs),
      pick,
    });
  }
  Ok(profiles)
}

/// The needs of the team's `profile` at `number`, from the body the server sent: the local body is the connection this
/// computer resolved, not what the profile asks.
fn team_profile(
  db: &ProjectDb,
  profile: ResourceId,
  number: VersionNumber,
  servers: &[McpNeed],
) -> Result<Result<Needs, Undecided>, StoreError> {
  let Some(cached) = db.cached_version::<AgentProfile>(profile, number)? else {
    return Ok(Err(Undecided::NotTeamForm));
  };
  let team = match serde_json::from_value::<TeamAgentProfile>(cached.server_body) {
    Ok(team) => team,
    Err(error) => {
      tracing::debug!(%profile, number = number.get(), %error, "a profile version is not in the team's form");
      return Ok(Err(Undecided::NotTeamForm));
    }
  };
  Ok(
    Needs::profile(profile, &team, servers)
      .map_err(|NeedsError::McpServerNotGiven(server)| Undecided::ChoosesMcpServer(server)),
  )
}

fn local_profile(
  db: &ProjectDb,
  profile: ResourceId,
  body: &AgentProfile,
) -> Result<(Result<Needs, Undecided>, Option<ConnectionId>), StoreError> {
  let own = body.connection();
  let provider = db.connection(own)?.and_then(|connection| match connection.kind() {
    orchester_core::connection::ConnectionKind::Agent(provider) => Some(*provider),
    orchester_core::connection::ConnectionKind::GitHub { .. } => None,
  });
  Ok(match provider {
    Some(provider) => (Ok(Needs::local_profile(profile, provider)), Some(own)),
    None => (Err(Undecided::ConnectionGone(own)), None),
  })
}

pub(super) fn tools(db: &ProjectDb) -> Result<Vec<Entry>, StoreError> {
  let mut tools = Vec::new();
  for resource in db.resources(ResourceKind::Tool)? {
    if let Some(version) = db.latest_version::<Tool>(resource.id())? {
      // A tool body names its secret by id until ORC-830, so there is no name to ask for.
      tools.push(Entry::new(&resource, version.number(), Ok(Needs::tool(Vec::new()))));
    }
  }
  Ok(tools)
}
