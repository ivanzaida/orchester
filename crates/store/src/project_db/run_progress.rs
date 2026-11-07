//! When a run last made progress, as the Orchestrator chat reads it to tell a run that stalls
//! (`orchester_core::orchestrator::task_event::no_progress`): its newest run event (a step change among others), its
//! newest transcript event (a tool call among others), its newest agent launch, and the newest decision on one of its
//! tool requests or answer to one of its questions (ORC-894: a run that waited for a person starts counting again once
//! they decided, not from before it asked). Read in one query, by the runs' keys and the invocations' index, without
//! loading the run.

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
         UNION ALL SELECT MAX(started_at) FROM invocations WHERE run_id = ?1
         UNION ALL SELECT MAX(decided_at) FROM tool_approvals WHERE run_id = ?1
         UNION ALL SELECT MAX(ended_at) FROM questions WHERE run_id = ?1 AND state = 'answered')",
      params![run.to_string()],
      |row| row.get::<_, Option<i64>>(0),
    )?;
    Ok(at.map(Timestamp::from_unix_millis))
  }

  /// When `run` first made progress after `after`, as [`Self::run_progressed_at`] counts it; `None` while it made none
  /// since: when a stall of the run settled.
  pub fn run_progressed_after(&self, run: RunId, after: Timestamp) -> Result<Option<Timestamp>, StoreError> {
    let at = self.connection.query_row(
      "SELECT MIN(at) FROM (
         SELECT MIN(at) AS at FROM run_events WHERE run_id = ?1 AND at > ?2
         UNION ALL SELECT MIN(at) FROM agent_events WHERE run_id = ?1 AND at > ?2
         UNION ALL SELECT MIN(started_at) FROM invocations WHERE run_id = ?1 AND started_at > ?2
         UNION ALL SELECT MIN(decided_at) FROM tool_approvals WHERE run_id = ?1 AND decided_at > ?2
         UNION ALL SELECT MIN(ended_at) FROM questions WHERE run_id = ?1 AND state = 'answered' AND ended_at > ?2)",
      params![run.to_string(), after.unix_millis()],
      |row| row.get::<_, Option<i64>>(0),
    )?;
    Ok(at.map(Timestamp::from_unix_millis))
  }
}
