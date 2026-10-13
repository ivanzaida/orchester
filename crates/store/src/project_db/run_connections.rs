//! The connection a step of a run continues on after its account reached its plan limit, chosen by a person for that
//! run only (`run_step_connections`); the step's profile keeps its own connection.

use orchester_core::{
  connection::AgentConnection,
  id::{ConnectionId, RunId},
  resource::{AgentProfile, StepKey},
  run::{Run, StepExecutor},
  store::{ConnectionStore, ResourceStore},
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// Runs `step` of `run` on `connection` from now on, in place of its profile's.
  pub fn set_step_connection(
    &mut self,
    run: RunId,
    step: &StepKey,
    connection: ConnectionId,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO run_step_connections (run_id, step_key, connection_id) VALUES (?1, ?2, ?3) \
       ON CONFLICT (run_id, step_key) DO UPDATE SET connection_id = excluded.connection_id",
      params![run.to_string(), step.as_str(), connection.to_string()],
    )?;
    Ok(())
  }

  /// The agent connection `step` of `run` runs on now: one a person chose for this run, else its profile's as the
  /// run pinned it; `None` when the step runs no agent, or the connection is gone or no agent CLI's.
  pub fn step_agent_connection(&self, run: &Run, step: &StepKey) -> Result<Option<AgentConnection>, StoreError> {
    let id = match self.step_connection(run.id(), step)? {
      Some(id) => id,
      None => {
        let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(step).map(|step| step.executor()) else {
          return Ok(None);
        };
        let Some(version) = self.version::<AgentProfile>(profile.resource, profile.number)? else {
          return Ok(None);
        };
        version.body().connection()
      }
    };
    Ok(
      self
        .connection(id)?
        .and_then(|connection| AgentConnection::try_from(connection).ok()),
    )
  }

  /// The connection `step` of `run` runs on in place of its profile's, if a person chose one.
  pub fn step_connection(&self, run: RunId, step: &StepKey) -> Result<Option<ConnectionId>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT connection_id FROM run_step_connections WHERE run_id = ?1 AND step_key = ?2",
        params![run.to_string(), step.as_str()],
        |row| row.get::<_, String>(0),
      )
      .optional()?
      .map(|id| parse("connection id", &id))
      .transpose()
  }
}
