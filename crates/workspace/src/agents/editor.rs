//! The window's profile actions. Validation and widening decisions are shared with the domain tools.
use std::path::Path;

use orchester_core::{connection::invocation::Platform, id::ResourceId, resource::AgentProfile};
use orchester_store::ProjectDb;

use super::data::{self, ProfileChange, ProfileEntry, ProjectAgents};
use crate::messages::{self, Message};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
  Save,
  Publish,
  KeepDraft,
}

/// A fixed candidate: confirmation never reads newly edited fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
  pub existing: Option<ProfileEntry>,
  pub profile: AgentProfile,
  pub action: Action,
  pub allowed: Vec<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
  Saved(ResourceId),
  Confirm(Vec<Message>),
}

/// Re-reads the stored profile and connection before every action, including confirmation.
pub fn apply(root: &Path, request: &Request) -> Result<Outcome, Message> {
  if request.existing.is_none() && request.action != Action::Save {
    return Err(Message::new("appMcp.errors.noDraft"));
  }
  let listed = ProjectAgents::load_in(root).map_err(|error| messages::store_error(&error))?;
  let id = request.existing.as_ref().map(|entry| entry.id);
  if let Some(expected) = &request.existing
    && listed.profile(expected.id) != Some(expected)
  {
    return Err(Message::new("errors.draftChanged"));
  }
  let connection = listed
    .connections
    .iter()
    .find(|c| c.id() == request.profile.connection())
    .ok_or_else(|| Message::new("errors.connectionRequired"))?;
  if request.action == Action::Publish && !connection.provider().runs_agents() {
    return Err(Message::new("appMcp.errors.connectionRunsNoAgents"));
  }
  request
    .profile
    .permissions()
    .enforced_on(connection.provider(), Platform::current())
    .map_err(|_| Message::new("agents.permissions.publish.notEnforceable"))?;
  if connection.provider().is_native()
    && (request.profile.permissions().interactive_tool_approvals()
      || request.profile.permissions().auto_approve()
      || !request.profile.mcp_servers().ids().is_empty())
  {
    return Err(Message::new("appMcp.errors.nativeNoSwitches"));
  }
  if request
    .profile
    .mcp_servers()
    .ids()
    .iter()
    .any(|id| !listed.mcp.iter().any(|server| server.id() == *id))
  {
    return Err(Message::new("appMcp.errors.mcpServersInvalid").with(
      "max",
      orchester_core::connection::mcp::McpServerSelection::MAX.to_string(),
    ));
  }
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let reasons = super::widening::reasons(&db, id, &request.profile, connection.provider())
    .map_err(|error| messages::store_error(&error))?;
  if reasons.iter().any(|reason| !request.allowed.contains(reason)) {
    return Ok(Outcome::Confirm(reasons));
  }
  let id = perform(root, id, request)?;
  Ok(Outcome::Saved(id))
}

fn perform(root: &Path, id: Option<ResourceId>, request: &Request) -> Result<ResourceId, Message> {
  let result = match (request.action, id) {
    (Action::Save, None) => data::apply_in(root, ProfileChange::Create(request.profile.clone())),
    (Action::Save, Some(id)) => data::apply_in(root, ProfileChange::SaveDraft(id, request.profile.clone())),
    (Action::Publish, Some(id)) => data::publish_checked_in(root, id, &request.profile).map(|_| id),
    (Action::KeepDraft, Some(id)) => data::apply_in(root, ProfileChange::KeepDraft(id)),
    (_, None) => return Err(Message::new("appMcp.errors.noDraft")),
  };
  result.map_err(|error| messages::store_error(&error))
}
