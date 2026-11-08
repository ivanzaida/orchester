//! What this computer reports to a shared project's server for its web editor to offer (`docs/architecture/server.md`,
//! "CLI kinds catalog", "Models per role", row 75): the agent roles its connections have (CLI kinds) and per role the
//! model ids its connections list. [`catalog`] is the one function that builds it, from the connections' kinds and
//! model lists alone, so no name, account, path, program, setting or key can reach a report.

use std::collections::BTreeMap;

use super::ConnectionRole;
use crate::{
  connection::{AgentConnection, Connection},
  resource::ModelId,
};

/// The most model ids one report may carry for one role.
pub const CATALOG_MODELS_PER_ROLE: usize = 512;

/// This computer's whole catalog for one project: the agent roles its connections have, sorted and once each, and per
/// role the model ids its connections list, sorted and once each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalCatalog {
  pub cli_kinds: Vec<&'static str>,
  pub models: BTreeMap<&'static str, Vec<ModelId>>,
}

/// The catalog of `connections`: their agent connections' roles and, per role, at most [`CATALOG_MODELS_PER_ROLE`]
/// model ids.
pub fn catalog(connections: &[Connection]) -> LocalCatalog {
  let mut models: BTreeMap<&'static str, Vec<ModelId>> = BTreeMap::new();
  for agent in connections
    .iter()
    // Other kinds are not agent connections; the conversion error is how they are left out.
    .filter_map(|connection| AgentConnection::try_from(connection.clone()).ok())
  {
    let ids = models.entry(ConnectionRole::of(agent.kind()).as_str()).or_default();
    ids.extend(agent.models().iter().map(|model| model.id().clone()));
  }
  // The map's keys are sorted and distinct.
  let cli_kinds = models.keys().copied().collect();
  for ids in models.values_mut() {
    ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    ids.dedup();
    ids.truncate(CATALOG_MODELS_PER_ROLE);
  }
  // A role whose connections list no model says nothing about models.
  models.retain(|_, ids| !ids.is_empty());
  LocalCatalog { cli_kinds, models }
}
