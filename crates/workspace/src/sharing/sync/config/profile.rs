//! A team agent profile on this computer. The server holds it in the team's form ([`TeamAgentProfile`]: a CLI kind, no
//! connection); runs and the Agents page read the local body ([`AgentProfile`]), which runs on a connection of this
//! computer: the one the person picked for the profile (`team_connection_picks`), else the only connection of its CLI
//! kind, else none, and the profile's own id stands in for the connection, which matches no connection here, so
//! preflight reports the connection missing. A version cached before the team configuration model is a local body
//! with the publisher's connection and is read as it is (ORC-845 removes that).

use orchester_api::SharedVersion;
use orchester_core::{
  connection::{AgentProvider, ConnectionKind},
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, TeamAgentProfile},
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::{body, cache_body};
use crate::sharing::SharingError;

/// Keeps `version` of an agent profile as the server has it, with the local body this computer runs it by.
pub(super) fn cache_profile(db: &mut ProjectDb, version: &SharedVersion) -> Result<(), SharingError> {
  let local = local_body(db, version.resource, &version.body)?;
  cache_body(db, version, local)
}

/// Writes again the local body of every cached version of the team profile `profile` in the team's form, with the
/// connection the person picked now; returns how many changed.
pub(crate) fn refresh(db: &mut ProjectDb, profile: ResourceId) -> Result<usize, StoreError> {
  let mut refreshed = 0;
  for version in db.versions::<AgentProfile>(profile)? {
    let Some(cached) = db.cached_version::<AgentProfile>(profile, version.number())? else {
      continue;
    };
    let Ok(team) = serde_json::from_value::<TeamAgentProfile>(cached.server_body) else {
      continue;
    };
    let local = AgentProfile::from_team(&team, connection_for(db, profile, team.cli())?);
    if local != cached.local_body {
      refreshed += usize::from(db.rewrite_local_body(profile, version.number(), &local)?);
    }
  }
  Ok(refreshed)
}

fn local_body(
  db: &ProjectDb,
  profile: ResourceId,
  server_body: &serde_json::Value,
) -> Result<AgentProfile, SharingError> {
  match serde_json::from_value::<TeamAgentProfile>(server_body.clone()) {
    Ok(team) => Ok(AgentProfile::from_team(&team, connection_for(db, profile, team.cli())?)),
    Err(_) => body(server_body.clone()),
  }
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
    _ => ConnectionId::from_uuid(profile.as_uuid()),
  })
}
