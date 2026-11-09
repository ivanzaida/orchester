//! The connection of a local profile (`Agents · v2` L1): a profile names the connection it runs on, so a profile whose
//! connection is gone, or that should run on another one of its CLI, takes the one the person chose as its next
//! version (`docs/design/team-configuration.md`, edge case 14: a local project keeps its model).

use std::path::Path;

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, VersionNumber},
};
use orchester_store::ProjectDb;

use super::{
  data::ProjectAgents,
  editor::{self, Action, Outcome, Request},
};
use crate::messages::Message;

/// Makes `connection` the one the local profile `profile` runs on, and publishes that as its next version, all or
/// nothing: a draft the save made is discarded when the publication is refused. Refused, with nothing changed, while
/// the profile has edits that are not published (they would go out with it), for a connection that is not an agent
/// connection of the profile's CLI, and when the change widens what its agents may do, which the editor asks about.
/// Moving to a connection on a named account is not such a widening while the profile's own connection is gone: no
/// account it ran on is left, so nothing moves away from one.
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
  let allowed = if current.is_none() {
    vec![Message::new("appMcp.widens.account")]
  } else {
    Vec::new()
  };
  let body = entry.profile.clone().with_connection(chosen.id());
  let save = Request {
    existing: Some(entry),
    profile: body.clone(),
    action: Action::Save,
    allowed: allowed.clone(),
    prompt_decided: false,
  };
  saved(&editor::apply(root, &save)?)?;
  publish_saved(root, profile, body.clone(), allowed).inspect_err(|_| discard_saved(root, profile, &body))
}

/// The draft the save made, published as the next version.
fn publish_saved(
  root: &Path,
  profile: ResourceId,
  body: AgentProfile,
  allowed: Vec<Message>,
) -> Result<VersionNumber, Message> {
  let draft = editor::entry_in(root, profile)?;
  let publish = Request {
    existing: Some(draft),
    profile: body,
    action: Action::Publish,
    allowed,
    prompt_decided: false,
  };
  saved(&editor::apply(root, &publish)?)?;
  editor::entry_in(root, profile)?
    .state
    .published()
    .ok_or_else(|| Message::new("errors.storage"))
}

/// A write that asks about what it widens did not happen: the person decides that in the editor.
fn saved(outcome: &Outcome) -> Result<(), Message> {
  match outcome {
    Outcome::Saved(_) => Ok(()),
    Outcome::Confirm(_) => Err(Message::new("agents.localConnection.widens")),
  }
}

/// Throws away the draft `body` that [`use_connection`] saved and could not publish, so that no half of the change is
/// left behind. A failure to discard is logged and not reported over the refusal that caused it: the hub shows the
/// draft that remains.
fn discard_saved(root: &Path, profile: ResourceId, body: &AgentProfile) {
  let discarded = ProjectDb::open(root).and_then(|mut db| db.discard_draft_matching(profile, body));
  if let Err(error) = discarded {
    tracing::error!(%error, "cannot discard the draft of a connection change that was refused");
  }
}
