//! A team agent profile on this computer. The server holds it in the team's form ([`TeamAgentProfile`]: a CLI kind, no
//! connection); runs and the Agents page read the local body ([`AgentProfile`]), which runs on a connection of this
//! computer: the one the person picked for the profile (`team_connection_picks`), else the only connection of its CLI
//! kind, else [`AgentProfile::unresolved_connection`], which matches no connection here, so the Agents page, publishing
//! and preflight say a connection must be picked. A version cached before the team configuration model is a local body
//! with the publisher's connection and is read as it is (ORC-845 removes that).

use orchester_api::SharedVersion;
use orchester_core::{
  connection::{AgentProvider, ConnectionKind},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ResourceKind, TeamAgentProfile},
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::Value;

use super::{body, cache_body};
use crate::sharing::SharingError;

/// Keeps `version` of an agent profile as the server has it, with the local body this computer runs it by.
pub(super) fn cache_profile(db: &mut ProjectDb, version: &SharedVersion) -> Result<(), SharingError> {
  let local = local_body(db, version.resource, &version.body)?;
  cache_body(db, version, local)
}

/// Writes again the local body of every cached version of every team profile of the project, for the connections of
/// this computer as they are now; returns how many changed. Called when connections are added or changed: the only
/// connection of a CLI kind, or none, is read from them.
pub(crate) fn refresh_all(db: &mut ProjectDb) -> Result<usize, StoreError> {
  let mut refreshed = 0;
  for profile in db.resources(ResourceKind::AgentProfile)? {
    refreshed += refresh(db, profile.id())?;
  }
  Ok(refreshed)
}

/// Writes again the local body of every cached version of the team profile `profile` in the team's form, with the
/// connection the person picked now; returns how many changed.
pub(crate) fn refresh(db: &mut ProjectDb, profile: ResourceId) -> Result<usize, StoreError> {
  let mut refreshed = 0;
  for version in db.versions::<AgentProfile>(profile)? {
    let Some(cached) = db.cached_version::<AgentProfile>(profile, version.number())? else {
      continue;
    };
    if is_local_form(&cached.server_body) {
      continue;
    }
    let team = match serde_json::from_value::<TeamAgentProfile>(cached.server_body) {
      Ok(team) => team,
      Err(error) => {
        tracing::warn!(%profile, version = version.number().get(), %error, "a cached team profile is unreadable, so its connection is not refreshed");
        continue;
      }
    };
    let local = AgentProfile::from_team(&team, connection_for(db, profile, team.cli())?);
    if local != cached.local_body {
      refreshed += usize::from(db.rewrite_local_body(profile, version.number(), &local)?);
    }
  }
  Ok(refreshed)
}

/// Whether `server_body` is a local profile body a version cached before the team configuration model holds: it names
/// a `connection`, which a team body never does (ORC-845 removes this with those versions).
fn is_local_form(server_body: &Value) -> bool {
  server_body.get("connection").is_some()
}

fn local_body(db: &ProjectDb, profile: ResourceId, server_body: &Value) -> Result<AgentProfile, SharingError> {
  if is_local_form(server_body) {
    return body(server_body.clone()); // ORC-845 removes this legacy fallback.
  }
  let team = serde_json::from_value::<TeamAgentProfile>(server_body.clone()).map_err(|error| {
    tracing::warn!(%profile, %error, "the server sent a team profile this version cannot read");
    SharingError::InvalidAnswer("team agent profile")
  })?;
  Ok(AgentProfile::from_team(&team, connection_for(db, profile, team.cli())?))
}

fn connection_for(db: &ProjectDb, profile: ResourceId, cli: AgentProvider) -> Result<ConnectionId, StoreError> {
  let connections = db.connections()?;
  if let Some(pick) = db.connection_pick(profile)?
    && connections.iter().any(|connection| connection.id() == pick.connection)
  {
    return Ok(pick.connection);
  }
  let mut running = connections
    .iter()
    .filter(|connection| *connection.kind() == ConnectionKind::Agent(cli));
  Ok(match (running.next(), running.next()) {
    (Some(only), None) => only.id(),
    _ => AgentProfile::unresolved_connection(profile),
  })
}
