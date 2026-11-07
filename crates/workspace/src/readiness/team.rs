//! The profiles and tools of a team project: what each asks of this computer, from the version this computer uses.

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, ResourceKind, TeamAgentProfile, Tool, Version},
  sharing::{McpNeed, Needs, NeedsError},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError, WaitingPublish};
use serde_json::Value;

use super::{
  needs::{Entry, Profile},
  standing::Undecided,
};
use crate::sharing::sync::{is_local_form, team_form};

/// The team's profiles, each read in the team's form. A version the server holds is read from the body it sent: the
/// local body is the connection this computer resolved, not what the profile asks. A version published here that waits
/// for the server has no such body, so it is read from the local body turned into the team's form exactly as
/// publishing does, and `servers` are the team's MCP servers as they stand.
pub(super) fn profiles(db: &ProjectDb, servers: &[McpNeed]) -> Result<Vec<Profile>, StoreError> {
  let waiting = db.waiting_publishes()?;
  let mut profiles = Vec::new();
  for resource in db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let needs = profile_needs(db, resource.id(), &version, (&waiting, servers))?;
    profiles.push(Profile {
      entry: Entry::new(&resource, version.number(), needs),
      pick: db.connection_pick(resource.id())?.map(|pick| pick.connection),
    });
  }
  Ok(profiles)
}

fn profile_needs(
  db: &ProjectDb,
  profile: ResourceId,
  version: &Version<AgentProfile>,
  (waiting, servers): (&[WaitingPublish], &[McpNeed]),
) -> Result<Result<Needs, Undecided>, StoreError> {
  Ok(body(db, profile, version, waiting)?.and_then(|team| with_servers(profile, &team, servers)))
}

/// What the team's `version` of `profile` says, in the team's form: read from the body the server sent, or, for one
/// this computer published that waits for the server, from its local body turned into the team's form.
pub(crate) fn body(
  db: &ProjectDb,
  profile: ResourceId,
  version: &Version<AgentProfile>,
  waiting: &[WaitingPublish],
) -> Result<Result<TeamAgentProfile, Undecided>, StoreError> {
  let number = version.number();
  if let Some(cached) = db.cached_version::<AgentProfile>(profile, number)? {
    return Ok(from_server(profile, cached.server_body));
  }
  if !waiting
    .iter()
    .any(|publish| publish.resource == profile && publish.number == number)
  {
    return Ok(Err(Undecided::NotTeamForm));
  }
  Ok(team_form(db, version.body())?.map_err(Undecided::CannotBePublished))
}

fn from_server(profile: ResourceId, server_body: Value) -> Result<TeamAgentProfile, Undecided> {
  if is_local_form(&server_body) {
    tracing::debug!(%profile, "a profile version cached before the team configuration model is not in the team's form");
    return Err(Undecided::NotTeamForm);
  }
  serde_json::from_value::<TeamAgentProfile>(server_body).map_err(|error| {
    tracing::warn!(%profile, %error, "the team's profile version is not a team profile body this version can read");
    Undecided::NotTeamForm
  })
}

fn with_servers(profile: ResourceId, team: &TeamAgentProfile, servers: &[McpNeed]) -> Result<Needs, Undecided> {
  Needs::profile(profile, team, servers)
    .map_err(|NeedsError::McpServerNotGiven(server)| Undecided::ChoosesMcpServer(server))
}

/// The team's tools. A team tool body names no secret by name yet, so a tool that holds none asks nothing. One that
/// holds a project secret by id is undecided: nothing says whether that secret has a value on this computer, and the
/// tool is not assumed ready (ORC-830 puts tools in the team's form).
pub(super) fn tools(db: &ProjectDb) -> Result<Vec<Entry>, StoreError> {
  let mut tools = Vec::new();
  for resource in db.resources(ResourceKind::Tool)? {
    if let Some(version) = db.latest_version::<Tool>(resource.id())? {
      let needs = match version.body().secret() {
        Some(secret) => Err(Undecided::TeamToolSecretById(secret)),
        None => Ok(Needs::tool(Vec::new())),
      };
      tools.push(Entry::new(&resource, version.number(), needs));
    }
  }
  Ok(tools)
}
