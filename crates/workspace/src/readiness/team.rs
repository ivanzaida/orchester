//! The profiles and tools of a team project: what each asks of this computer, from the version this computer uses.

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, Resource, ResourceKind, TeamAgentProfile, TeamReviewer, Tool, Version},
  sharing::{McpNeed, Needs, NeedsError},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError, WaitingPublish};
use serde_json::Value;

use super::{
  needs::{Entry, Profile, Reviewer},
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

/// The team's reviewers. A reviewer's local body is its team body (it names no connection), so a version cached from
/// the server and one published here that waits for it are read alike. The connection is the pick of the kind the
/// body names; a pick of the other kind reads as none.
pub(super) fn reviewers(db: &ProjectDb) -> Result<Vec<Reviewer>, StoreError> {
  let mut reviewers = Vec::new();
  for resource in db.resources(ResourceKind::TeamReviewer)? {
    let version = match db.latest_version::<TeamReviewer>(resource.id()) {
      Ok(Some(version)) => version,
      Ok(None) => continue,
      Err(StoreError::Corrupt { .. }) => {
        reviewers.push(unreadable_reviewer(db, &resource)?);
        continue;
      }
      Err(other) => return Err(other),
    };
    let language = version.body().cli().is_some();
    reviewers.push(Reviewer {
      pick: if language {
        db.connection_pick(resource.id())?.map(|pick| pick.connection)
      } else {
        None
      },
      decision_pick: if language {
        None
      } else {
        db.decision_pick(resource.id())?
      },
      entry: Entry::new(
        &resource,
        version.number(),
        Ok(Needs::reviewer(resource.id(), version.body())),
      ),
    });
  }
  Ok(reviewers)
}

/// A reviewer whose newest version is not a team reviewer body: one cached before ORC-887, which names a connection and
/// is not read. Republishing it names a CLI kind instead.
fn unreadable_reviewer(db: &ProjectDb, resource: &Resource) -> Result<Reviewer, StoreError> {
  tracing::warn!(reviewer = %resource.id(), "a cached team reviewer version names a connection, so it is not read");
  let number = db
    .latest_version_number(resource.id())?
    .ok_or(StoreError::ResourceNotFound(resource.id()))?;
  Ok(Reviewer {
    entry: Entry::new(resource, number, Err(Undecided::NotTeamForm)),
    pick: None,
    decision_pick: None,
  })
}

fn profile_needs(
  db: &ProjectDb,
  profile: ResourceId,
  version: &Version<AgentProfile>,
  (waiting, servers): (&[WaitingPublish], &[McpNeed]),
) -> Result<Result<Needs, Undecided>, StoreError> {
  let number = version.number();
  if let Some(cached) = db.cached_version::<AgentProfile>(profile, number)? {
    return Ok(from_server(profile, cached.server_body, servers));
  }
  if !waiting
    .iter()
    .any(|publish| publish.resource == profile && publish.number == number)
  {
    return Ok(Err(Undecided::NotTeamForm));
  }
  Ok(match team_form(db, version.body())? {
    Ok(team) => with_servers(profile, &team, servers),
    Err(error) => Err(Undecided::CannotBePublished(error)),
  })
}

fn from_server(profile: ResourceId, server_body: Value, servers: &[McpNeed]) -> Result<Needs, Undecided> {
  if is_local_form(&server_body) {
    tracing::debug!(%profile, "a profile version cached before the team configuration model is not in the team's form");
    return Err(Undecided::NotTeamForm);
  }
  let team = serde_json::from_value::<TeamAgentProfile>(server_body).map_err(|error| {
    tracing::warn!(%profile, %error, "the team's profile version is not a team profile body this version can read");
    Undecided::NotTeamForm
  })?;
  with_servers(profile, &team, servers)
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
