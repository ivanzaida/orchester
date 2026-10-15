//! `list_profiles`, `get_profile` and `publish_profile`: a project's agent profiles as Agents lists and
//! opens them, and a draft published as its Publish does, with the editor's own rule that a connection
//! that runs no agents cannot publish.

use orchester_core::{
  connection::{AgentConnection, mcp::McpServer},
  id::{ProjectId, ResourceId},
  permission::{AgentPermissions, CommandPermission},
  resource::{AgentProfile, PublishError},
};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{
    Translate,
    across::{InProjects, across},
  },
  connections::agent_by_id,
  widening,
};
use crate::{
  agents::{
    data::{ProfileEntry, ProjectAgents, VersionState, publish_checked_in},
    form::stored_effort,
  },
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  drafts::revision_of,
  links::Link,
  messages::{self, Message},
  workspace::OpenProject,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InProfile {
  /// The project's id, or its name.
  pub project: String,
  /// The profile's id, or its role.
  pub profile: String,
}

pub async fn list_profiles(services: &Services, args: InProjects, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let read = blocking(move || across(&services, args.project.as_deref(), |_, db| Ok(ProjectAgents::load(db)?))).await?;
  Ok(read.json("profiles", translate, |open, agents| {
    agents
      .profiles
      .iter()
      .map(|entry| {
        let connection = agent_by_id(&agents.connections, entry.profile.connection());
        json!({
          "id": entry.id.to_string(),
          "role": entry.profile.role().as_str(),
          "connection": connection.map(|connection| connection.name().as_str()),
          "model": entry.profile.model().as_str(),
          "effort": stored_effort(&entry.profile),
          "state": state_json(entry.state),
          "link": Link::Profile(open.project.id(), entry.id).path(),
        })
      })
      .collect()
  }))
}

pub async fn get_profile(services: &Services, args: InProfile) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let id = profile_named(&ProjectAgents::load_in(&open.repo_root)?, &args.profile)?.id;
    Ok(json!({ "profile": profile_with_revision(&open, id)? }))
  })
  .await
}

/// Profile `id` of `open` as get_profile shows it, with its revision ([`crate::drafts::draft_revision`]) read
/// before the profile: a save made meanwhile shows in the profile, so a save on that revision is refused rather
/// than going over what was not shown.
pub(super) fn profile_with_revision(open: &OpenProject, id: ResourceId) -> Result<Value, ToolError> {
  let revision = revision_of::<AgentProfile, _>(&ProjectDb::open(&open.repo_root)?, id)?;
  let agents = ProjectAgents::load_in(&open.repo_root)?;
  let entry = agents
    .profile(id)
    .ok_or_else(|| ToolError::not_found(Missing::Profile, id.to_string()))?;
  let mut value = profile_json(&agents, entry, open.project.id());
  value["revision"] = json!(revision);
  Ok(value)
}

pub async fn publish_profile(services: &Services, args: InProfile) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let agents = ProjectAgents::load_in(&open.repo_root)?;
    let entry = profile_named(&agents, &args.profile)?;
    let id = entry.id;
    services.draft_decided::<orchester_core::resource::AgentProfile>(&ProjectDb::open(&open.repo_root)?, id)?;
    if let Some(connection) = agent_by_id(&agents.connections, entry.profile.connection()) {
      // The editor offers no Publish on a connection that runs no agents.
      if !connection.provider().runs_agents() {
        return Err(ToolError::refused(
          "connection_runs_no_agents",
          vec![Message::new("appMcp.errors.connectionRunsNoAgents")],
        ));
      }
      let db = ProjectDb::open(&open.repo_root)?;
      widening::check_profile(&services, &db, (Some(id), &entry.profile), connection.provider())?;
    }
    // Published only while the draft is the one just checked.
    publish_checked_in(&open.repo_root, id, &entry.profile).map_err(|error| publish_error(&error))?;
    services
      .changes
      .record(open.project.id(), crate::tasks::outside::Outside::Profiles);
    let agents = ProjectAgents::load_in(&open.repo_root)?;
    let entry = agents
      .profile(id)
      .ok_or_else(|| ToolError::not_found(Missing::Profile, id.to_string()))?;
    Ok(json!({ "profile": profile_json(&agents, entry, open.project.id()) }))
  })
  .await
}

/// The profile of `agents` named by its id or, ignoring case, its role.
pub(super) fn profile_named<'a>(agents: &'a ProjectAgents, named: &str) -> Result<&'a ProfileEntry, ToolError> {
  let lowered = named.to_lowercase();
  // An id names one profile for certain; a role could also be another profile's id.
  let by_id = agents.profiles.iter().find(|entry| entry.id.to_string() == named);
  by_id
    .or_else(|| {
      agents
        .profiles
        .iter()
        .find(|entry| entry.profile.role().as_str().to_lowercase() == lowered)
    })
    .ok_or_else(|| ToolError::not_found(Missing::Profile, named))
}

/// A profile of `project` as `get_profile` shows it, with its link.
pub(super) fn profile_json(agents: &ProjectAgents, entry: &ProfileEntry, project: ProjectId) -> Value {
  let profile = &entry.profile;
  let connection: Option<&AgentConnection> = agent_by_id(&agents.connections, profile.connection());
  let servers: Vec<Value> = profile
    .mcp_servers()
    .ids()
    .iter()
    .map(|id| {
      let server: Option<&McpServer> = agents.mcp.iter().find(|server| server.id() == *id);
      json!({ "id": id.to_string(), "name": server.map(|server| server.name().to_string()) })
    })
    .collect();
  let steps: Vec<Value> = agents
    .steps
    .iter()
    .filter(|step| step.profile == entry.id)
    .map(|step| json!({ "pipeline": &*step.pipeline, "step": &*step.step }))
    .collect();
  json!({
    "id": entry.id.to_string(),
    "role": profile.role().as_str(),
    "responsibilities": profile.responsibilities(),
    "instructions": profile.instructions().as_str(),
    "connection": connection.map(|connection| json!({
      "id": connection.id().to_string(),
      "name": connection.name().as_str(),
      "provider": connection.provider(),
    })),
    "model": profile.model().as_str(),
    "effort": stored_effort(profile),
    "permissions": permissions_json(profile.permissions()),
    "mcp_servers": servers,
    "state": state_json(entry.state),
    "used_by_steps": steps,
    "link": Link::Profile(project, entry.id).path(),
  })
}

/// Permissions as `save_profile_draft` takes them.
pub(super) fn permissions_json(permissions: &AgentPermissions) -> Value {
  let (commands, allowed) = match permissions.commands() {
    CommandPermission::AskBefore => ("ask_before", Vec::new()),
    CommandPermission::Allowed(list) => ("allowed", list.entries().iter().map(ToString::to_string).collect()),
    CommandPermission::AnyInWorktree => ("any_in_worktree", Vec::new()),
  };
  let folders: Vec<String> = permissions
    .writable_folders()
    .entries()
    .iter()
    .map(ToString::to_string)
    .collect();
  json!({
    "repository": permissions.repository(),
    "writable_folders": folders,
    "commands": commands,
    "allowed_commands": allowed,
    "network": permissions.network(),
    "ask_before_tool_calls": permissions.interactive_tool_approvals(),
    "approve_automatically": permissions.auto_approve(),
  })
}

pub(super) fn state_json(state: VersionState) -> Value {
  let name = match state {
    VersionState::Draft { .. } => "draft",
    VersionState::StaleDraft { .. } => "stale_draft",
    VersionState::Published(_) => "published",
  };
  json!({ "state": name, "published_version": state.published().map(|version| version.get()) })
}

fn publish_error(error: &StoreError) -> ToolError {
  let code = match error {
    StoreError::NoDraft(_) => return no_draft(),
    StoreError::Publish(PublishError::StaleDraft { .. }) => "stale_draft",
    StoreError::DraftChanged(_) => "draft_changed",
    StoreError::Publish(PublishError::ConnectionMissing) => "connection_missing",
    StoreError::Publish(PublishError::Permissions(_)) => "permissions_not_enforced",
    StoreError::Publish(PublishError::VersionLimit) => "version_limit",
    _ => "project_data",
  };
  ToolError::refused(code, vec![messages::store_error(error)])
}

/// Publishing found no draft: the latest version is current.
pub(super) fn no_draft() -> ToolError {
  ToolError::refused("no_draft", vec![Message::new("appMcp.errors.noDraft")])
}
