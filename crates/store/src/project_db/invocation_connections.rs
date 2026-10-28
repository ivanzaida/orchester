//! The agent connection each agent invocation started on, with its name then (`invocation_connections`), written once
//! by the runner before the agent starts; and the agent providers usage names: the project's agent connections as the
//! Connections page lists them, then those gone since that a record still names, by the name they last had.

use std::sync::Arc;

use orchester_core::{
  connection::ConnectionKind,
  id::{ConnectionId, InvocationId},
  store::ConnectionStore,
  usage::{ProviderName, UsedProvider},
};
use rusqlite::params;

use super::ProjectDb;
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// Records that `invocation` starts on `connection`.
  pub fn save_invocation_connection(
    &mut self,
    invocation: InvocationId,
    connection: &UsedProvider,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO invocation_connections (invocation_id, connection_id, name) VALUES (?1, ?2, ?3)",
      params![invocation.to_string(), connection.id.to_string(), &*connection.name],
    )?;
    Ok(())
  }

  /// The agent providers usage names: the agent connections by name, as the Connections page lists them, then the
  /// connections an invocation or a review recorded that are gone, each by the name it was last recorded with.
  pub(super) fn usage_providers(&self) -> Result<Vec<ProviderName>, StoreError> {
    let mut providers: Vec<ProviderName> = self
      .connections()?
      .into_iter()
      .filter_map(|connection| match connection.kind() {
        ConnectionKind::Agent(_) => Some(ProviderName {
          id: connection.id(),
          name: Arc::from(connection.name().as_str()),
          removed: false,
        }),
        ConnectionKind::GitHub { .. } => None,
      })
      .collect();
    let mut statement = self.connection.prepare(
      "SELECT connection_id, name FROM ( \
         SELECT recorded.connection_id, recorded.name, invocations.started_at AS at FROM invocation_connections AS recorded \
           JOIN invocations ON invocations.id = recorded.invocation_id \
         UNION ALL \
         SELECT connection_id, connection_name, ended_at FROM reviewer_usage WHERE connection_id IS NOT NULL \
       ) WHERE connection_id NOT IN (SELECT id FROM connections) ORDER BY at DESC",
    )?;
    let recorded = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    let mut removed: Vec<ProviderName> = Vec::new();
    for (id, name) in recorded {
      let id: ConnectionId = parse("connection id", &id)?;
      // Newest first: the first row of a connection holds the name it last had.
      if !removed.iter().any(|known| known.id == id) {
        removed.push(ProviderName {
          id,
          name: name.into(),
          removed: true,
        });
      }
    }
    removed.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    providers.extend(removed);
    Ok(providers)
  }
}
