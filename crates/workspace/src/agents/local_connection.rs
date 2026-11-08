//! The connection of a local profile (`Agents · v2` L1): a profile names the connection it runs on, so a profile whose
//! connection is gone, or that should run on another one of its CLI, takes the one the person chose as its next
//! version (`docs/design/team-configuration.md`, edge case 14: a local project keeps its model).

use std::path::Path;

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::VersionNumber,
};

use super::{
  data::ProjectAgents,
  editor::{self, Action, Outcome, Request},
};
use crate::messages::Message;

/// Makes `connection` the one the local profile `profile` runs on, and publishes that as its next version. Refused,
/// with nothing changed, while the profile has edits that are not published (they would go out with it), for a
/// connection that is not an agent connection of the profile's CLI, and when the change widens what its agents may do,
/// which the editor asks about.
pub fn use_connection(root: &Path, profile: ResourceId, connection: ConnectionId) -> Result<VersionNumber, Message> {
  let listed = ProjectAgents::load_in(root).map_err(|error| crate::messages::store_error(&error))?;
  let entry = listed
    .profile(profile)
    .cloned()
    .ok_or_else(|| Message::new("agents.localConnection.gone"))?;
  if entry.state.has_draft() {
    return Err(Message::new("agents.localConnection.draftFirst"));
  }
  let current = listed
    .connections
    .iter()
    .find(|found| found.id() == entry.profile.connection());
  let cli = current.map_or_else(
    || AgentProvider::cli_of_model(entry.profile.model().as_str()),
    |found| Some(found.provider()),
  );
  let chosen = listed
    .connections
    .iter()
    .find(|found| found.id() == connection && Some(found.provider()) == cli)
    .ok_or_else(|| Message::new("agents.localConnection.notOfKind"))?;
  let body = entry.profile.clone().with_connection(chosen.id());
  let save = Request {
    existing: Some(entry),
    profile: body.clone(),
    action: Action::Save,
    allowed: Vec::new(),
    prompt_decided: false,
  };
  match editor::apply(root, &save)? {
    Outcome::Saved(_) => {}
    Outcome::Confirm(_) => return Err(Message::new("agents.localConnection.widens")),
  }
  let draft = editor::entry_in(root, profile)?;
  let publish = Request {
    existing: Some(draft),
    profile: body,
    action: Action::Publish,
    allowed: Vec::new(),
    prompt_decided: false,
  };
  match editor::apply(root, &publish)? {
    Outcome::Saved(_) => {}
    Outcome::Confirm(_) => return Err(Message::new("agents.localConnection.widens")),
  }
  editor::entry_in(root, profile)?
    .state
    .published()
    .ok_or_else(|| Message::new("errors.storage"))
}
