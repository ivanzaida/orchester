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
  /// The person confirmed replacing the latest version with this stale draft.
  PublishStale,
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
  if matches!(request.action, Action::Publish | Action::PublishStale) && !connection.provider().runs_agents() {
    return Err(Message::new("appMcp.errors.connectionRunsNoAgents"));
  }
  request
    .profile
    .permissions()
    .enforced_on(connection.provider(), Platform::current())
    .map_err(|problem| super::permissions::not_enforceable(&problem))?;
  if connection.provider().is_native()
    && (request.profile.permissions().interactive_tool_approvals()
      || request.profile.permissions().auto_approve()
      || !request.profile.mcp_servers().ids().is_empty())
  {
    return Err(Message::new("appMcp.errors.nativeNoSwitches"));
  }
  super::servers::check(
    &listed.mcp,
    request.existing.as_ref().map(|entry| &entry.profile),
    &request.profile,
  )?;
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let reasons = super::widening::reasons(&db, id, &request.profile, connection.provider())
    .map_err(|error| messages::store_error(&error))?;
  if reasons.iter().any(|reason| !request.allowed.contains(reason)) {
    return Ok(Outcome::Confirm(reasons));
  }
  let id = perform(root, request)?;
  Ok(Outcome::Saved(id))
}

fn perform(root: &Path, request: &Request) -> Result<ResourceId, Message> {
  let result = match (request.action, &request.existing) {
    (Action::Save, None) => data::apply_in(root, ProfileChange::Create(request.profile.clone())),
    (Action::Save, Some(expected)) => {
      data::save_draft_checked_in(root, expected, request.profile.clone()).map(|()| expected.id)
    }
    (Action::Publish, Some(expected)) => {
      data::publish_checked_in(root, expected.id, &request.profile).map(|_| expected.id)
    }
    (Action::PublishStale, Some(expected)) if request.profile == expected.profile => {
      data::publish_stale_checked_in(root, expected).map(|_| expected.id)
    }
    (Action::PublishStale, Some(_)) => return Err(Message::new("errors.draftChanged")),
    (_, None) => return Err(Message::new("appMcp.errors.noDraft")),
  };
  result.map_err(|error| messages::store_error(&error))
}
