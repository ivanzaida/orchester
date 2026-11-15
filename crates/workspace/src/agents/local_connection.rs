//! What a local profile runs on on this computer (`Agents on accounts` C1 `RwzH2`, `docs/design/agent-accounts.md`,
//! "Agents run on accounts", decision 4): the person's choice is kept beside the profile and applies at once, with no
//! new version. A choice that needs another model, or another CLI, is not made here: the panel holds it and *Save*
//! publishes it ([`super::local_edit`]).

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ResourceId},
  resource::AgentProfile,
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ConnectionPick, ProjectDb};

use super::form::listed_models;
use crate::messages::{self, Message};

/// Makes `connection` the one the local profile `profile` runs on on this computer, in place of what it ran on here
/// before. Its published version stays as it is, and so does a draft of it. Refused, with nothing changed, for a
/// profile that was never published, for a connection that is not an agent connection of the profile's CLI or runs
/// no agents in this version, and for one that lists models without the profile's among them: the profile's model,
/// settings and permissions were made for its CLI, and a model is the version's to name.
pub fn run_here(root: &Path, profile: ResourceId, connection: ConnectionId) -> Result<(), Message> {
  let store = |error| messages::store_error(&error);
  let mut db = ProjectDb::open(root).map_err(store)?;
  let published = db
    .latest_version::<AgentProfile>(profile)
    .map_err(store)?
    .ok_or_else(|| Message::new("agents.localConnection.gone"))?;
  let body = published.body();
  let agent = |id| -> Result<Option<AgentConnection>, Message> {
    Ok(
      db.connection(id)
        .map_err(store)?
        .and_then(|found| AgentConnection::try_from(found).ok()),
    )
  };
  let cli = agent(body.connection())?.map_or_else(
    || AgentProvider::cli_of_model(body.model().as_str()),
    |own| Some(own.provider()),
  );
  let chosen = agent(connection)?
    .filter(|found| Some(found.provider()) == cli)
    .ok_or_else(|| Message::new("agents.localConnection.notOfKind"))?;
  if !chosen.provider().runs_agents() {
    return Err(Message::new("appMcp.errors.connectionRunsNoAgents"));
  }
  let listed = listed_models(&chosen);
  if !listed.is_empty() && !listed.contains(body.model()) {
    return Err(Message::new("agents.localEdit.modelNotOffered"));
  }
  let pick = ConnectionPick {
    connection: chosen.id(),
    account: chosen.plan_account(),
  };
  db.set_connection_pick(profile, pick).map_err(store)
}

/// Throws away the draft `body` that the hub's Save ([`super::local_edit::save`]) saved and could not publish, so
/// that no half of the change is left behind. A failure to discard is logged and not reported over the refusal that
/// caused it: the hub shows the draft that remains.
pub(super) fn discard_saved(root: &Path, profile: ResourceId, body: &AgentProfile) {
  let discarded = ProjectDb::open(root).and_then(|mut db| db.discard_draft_matching(profile, body));
  if let Err(error) = discarded {
    tracing::error!(%error, "cannot discard the draft of a change that was refused");
  }
}
