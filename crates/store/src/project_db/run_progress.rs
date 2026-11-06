//! When a run last made progress, as the Orchestrator chat reads it to tell a run that stalls
//! (`orchester_core::orchestrator::task_event::no_progress`): its newest run event (a step change among others), its
//! newest transcript event (a tool call among others) and its newest agent launch. Read in one query, by the runs'
//! keys and the invocations' index, without loading the run.

use orchester_core::{id::RunId, time::Timestamp};
use rusqlite::params;

use super::ProjectDb;
use crate::StoreError;

impl ProjectDb {
  /// When `run` last made progress; `None` while it has recorded nothing.
  pub fn run_progressed_at(&self, run: RunId) -> Result<Option<Timestamp>, StoreError> {
    let at = self.connection.query_row(
      "SELECT MAX(at) FROM (
         SELECT (SELECT at FROM run_events WHERE run_id = ?1 ORDER BY sequence DESC LIMIT 1) AS at
         UNION ALL SELECT (SELECT at FROM agent_events WHERE run_id = ?1 ORDER BY sequence DESC LIMIT 1)
         UNION ALL SELECT MAX(started_at) FROM invocations WHERE run_id = ?1)",
      params![run.to_string()],
      |row| row.get::<_, Option<i64>>(0),
    )?;
    Ok(at.map(Timestamp::from_unix_millis))
  }
}
