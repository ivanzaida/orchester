//! A team project's agent profiles through the tools (`docs/design/local-and-team-parity.md`): they are the team's
//! resources, read from the team's configuration this session read, never this computer's own rows. `list_profiles`
//! lists them, `get_profile` opens one as its editor shows it, `save_profile_draft` publishes its next version on the
//! team server at once ([`super::team_profile_save`]) and `publish_profile` answers that it is published: the team
//! keeps no draft on a computer.

use std::sync::Arc;

use orchester_core::{
  connection::AgentConnection,
  id::{ConnectionId, ProjectId, ResourceId},
  resource::{AgentProfile, ResourceKind, TeamAgentProfile, VersionNumber},
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{super::readiness_json, connections::agent_by_id, profiles::permissions_json, runs_on::Accounts};
use crate::{
  agents::{data::ProjectAgents, form::stored_effort},
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::Link,
  readiness::ProjectReadiness,
  team_config::TeamConfig,
  workspace::OpenProject,
};

/// A team project the profile tools act on: its folder and the team's configuration this session read.
pub(super) struct TeamProject {
  pub(super) open: OpenProject,
  pub(super) config: Arc<TeamConfig>,
}

/// Project `named` when it is a team project, with the team's configuration; `None` for a local project. Refused
/// while the configuration is being read or was never read this session (`team_reading`, `team_unreachable`).
pub(super) async fn team_of(services: &Services, named: &str) -> Result<Option<TeamProject>, ToolError> {
  let (services, named) = (services.clone(), named.to_owned());
  blocking(move || {
    let open = services.project(&named)?;
    if ProjectDb::open(&open.repo_root)?.sharing()?.is_none() {
      return Ok(None);
    }
    let config = services.workspace.team_read(&open.repo_root).loaded()?;
    Ok(Some(TeamProject { open, config }))
  })
  .await
}

/// A team profile with its latest version.
pub(super) struct TeamProfile {
  pub(super) id: ResourceId,
  pub(super) number: VersionNumber,
  pub(super) body: TeamAgentProfile,
}

/// The team's profiles in `config` that are not archived and whose latest version this computer reads, by role.
pub(super) fn profiles(config: &TeamConfig) -> Vec<TeamProfile> {
  config
    .resources(ResourceKind::AgentProfile)
    .into_iter()
    .filter_map(|resource| {
      let body = resource.body::<TeamAgentProfile>()?.ok()?;
      Some(TeamProfile {
        id: resource.id,
        number: resource.number()?,
        body,
      })
    })
    .collect()
}

/// The team profile of `config` named by its id or, ignoring case, its role.
pub(super) fn named(config: &TeamConfig, named: &str) -> Result<TeamProfile, ToolError> {
  let mut all = profiles(config);
  let lowered = named.to_lowercase();
  let at = all
    .iter()
    .position(|profile| profile.id.to_string() == named)
    .or_else(|| {
      all
        .iter()
        .position(|profile| profile.body.role().as_str().to_lowercase() == lowered)
    })
    .ok_or_else(|| ToolError::not_found(Missing::Profile, named))?;
  Ok(all.swap_remove(at))
}

/// What a team profile is read with: the project's agents and the accounts, and what is ready on this computer.
pub(super) struct Reads<'a> {
  pub(super) db: &'a ProjectDb,
  pub(super) agents: &'a ProjectAgents,
  pub(super) accounts: &'a Accounts,
  pub(super) readiness: &'a ProjectReadiness,
}

impl Reads<'_> {
  /// The connection this computer picked for team profile `id`, which its runs here go on.
  fn here(&self, id: ResourceId) -> Result<(Option<&AgentConnection>, Value), ToolError> {
    let pick = self.db.connection_pick(id)?;
    let connection = pick
      .as_ref()
      .and_then(|pick| agent_by_id(&self.agents.connections, pick.connection));
    let runs_on = connection.map(|connection| {
      self
        .accounts
        .runs_on_json(connection, pick.as_ref().and_then(|pick| pick.account))
    });
    Ok((connection, json!(runs_on)))
  }

  /// `profile` as `list_profiles` lists it.
  pub(super) fn listed(&self, project: ProjectId, profile: &TeamProfile) -> Result<Value, ToolError> {
    let (connection, runs_on) = self.here(profile.id)?;
    let local = local_form(profile, connection);
    Ok(json!({
      "id": profile.id.to_string(),
      "role": profile.body.role().as_str(),
      "team": true,
      "cli": profile.body.cli(),
      "runs_on": runs_on,
      "model": profile.body.model().as_str(),
      "effort": stored_effort(&local),
      "state": { "state": "published", "published_version": profile.number.get() },
      "readiness": readiness_json::resource_of(self.readiness, profile.id),
      "link": Link::Profile(project, profile.id).path(),
    }))
  }

  /// `profile` as `get_profile` shows it: every field of its latest version, the team's resources it chooses by
  /// `{id, name}`, and its `revision`, the version's number, which `save_profile_draft`'s `expected_revision` takes.
  pub(super) fn shown(
    &self,
    project: ProjectId,
    config: &TeamConfig,
    profile: &TeamProfile,
  ) -> Result<Value, ToolError> {
    let mut shown = self.listed(project, profile)?;
    let named = |ids: &[ResourceId]| -> Vec<Value> {
      ids
        .iter()
        .map(|id| json!({ "id": id.to_string(), "name": config.name(*id) }))
        .collect()
    };
    let body = &profile.body;
    let extra = json!({
      "responsibilities": body.responsibilities(),
      "instructions": body.instructions().as_str(),
      "permissions": permissions_json(body.permissions()),
      "mcp_servers": named(body.mcp_servers()),
      "instruction_resources": named(body.instruction_resources()),
      "skills": named(body.skills()),
      "tools": named(body.tools()),
      "secrets": body.secrets().iter().map(ToString::to_string).collect::<Vec<_>>(),
      "revision": profile.number.get().to_string(),
    });
    if let (Some(shown), Some(extra)) = (shown.as_object_mut(), extra.as_object()) {
      shown.extend(extra.clone());
    }
    Ok(shown)
  }
}

/// Team profile `id` of `open`'s project as `get_profile` answers it, read now.
pub(super) async fn shown_now(services: &Services, open: &OpenProject, id: ResourceId) -> Result<Value, ToolError> {
  let (services, open) = (services.clone(), open.clone());
  blocking(move || {
    let config = services.workspace.team_read(&open.repo_root).loaded()?;
    let profile = named(&config, &id.to_string())?;
    let db = ProjectDb::open(&open.repo_root)?;
    let (agents, accounts) = (ProjectAgents::load(&db)?, Accounts::load(&services)?);
    let readiness = readiness_json::read(&services, &db, &open.repo_root)?;
    let reads = Reads {
      db: &db,
      agents: &agents,
      accounts: &accounts,
      readiness: &readiness,
    };
    Ok(json!({ "profile": reads.shown(open.project.id(), &config, &profile)? }))
  })
  .await
}

/// The team profiles of the project at `open` as `list_profiles` lists them, leaving out those `listed` lists already;
/// none while the team's configuration was not read this session.
pub(super) fn listed_in(
  services: &Services,
  (open, db): (&OpenProject, &ProjectDb),
  readiness: &ProjectReadiness,
  listed: &[ResourceId],
) -> Result<Vec<Value>, ToolError> {
  if db.sharing()?.is_none() {
    return Ok(Vec::new());
  }
  let Some(config) = services.workspace.team_read(&open.repo_root).config().cloned() else {
    return Ok(Vec::new());
  };
  let (agents, accounts) = (ProjectAgents::load(db)?, Accounts::load(services)?);
  let reads = Reads {
    db,
    agents: &agents,
    accounts: &accounts,
    readiness,
  };
  profiles(&config)
    .iter()
    .filter(|profile| !listed.contains(&profile.id))
    .map(|profile| reads.listed(open.project.id(), profile))
    .collect()
}

/// `profile` in the local form, on `connection` (the one this computer picked) or, without one, on none yet.
pub(super) fn local_form(profile: &TeamProfile, connection: Option<&AgentConnection>) -> AgentProfile {
  let connection = connection.map_or_else(
    || AgentProfile::unresolved_connection(profile.id),
    |connection| connection.id(),
  );
  AgentProfile::from_team(&profile.body, connection)
}

/// The ids of the team MCP servers of `config`, as a local profile's selection names them.
pub(super) fn team_server_ids(config: &TeamConfig) -> Vec<ResourceId> {
  config
    .resources(ResourceKind::McpServer)
    .iter()
    .map(|server| server.id)
    .collect()
}

/// The team MCP server of `config` named by its id or, ignoring case, its name, as a local profile's selection names
/// it.
pub(super) fn server_named(config: &TeamConfig, named: &str) -> Result<ConnectionId, ToolError> {
  config
    .resources(ResourceKind::McpServer)
    .iter()
    .find(|server| server.id.to_string() == named || server.name.as_str().to_lowercase() == named.to_lowercase())
    .map(|server| ConnectionId::from_uuid(server.id.as_uuid()))
    .ok_or_else(|| ToolError::not_found(Missing::McpServer, named))
}

/// The team resource of `kind` in `config` named by its id, its slug or, ignoring case, its name.
pub(super) fn resource_named(
  config: &TeamConfig,
  kind: ResourceKind,
  named: &str,
  missing: Missing,
) -> Result<ResourceId, ToolError> {
  config
    .resources(kind)
    .iter()
    .find(|found| {
      found.id.to_string() == named || found.slug == named || found.name.as_str().to_lowercase() == named.to_lowercase()
    })
    .map(|found| found.id)
    .ok_or_else(|| ToolError::not_found(missing, named))
}
