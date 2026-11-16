//! The plan limits that hold the Orchestrator's turns (`orchester_core::orchestrator::hold`): the runner keeps that
//! the account a turn ran on is at its plan limit, as the turn's CLI reported, and forgets it once that account
//! answers again; the follower reads it before it starts a turn by itself.

use orchester_core::{id::AgentAccountId, orchestrator::PlanHold, time::Timestamp};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::StoreError;

impl ProjectDb {
  /// Keeps that `account` is at its plan limit, in place of what was kept of it before.
  pub fn hold_orchestrator(&mut self, account: AgentAccountId, hold: PlanHold) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO orchestrator_plan_holds (account_id, since, resets_at) VALUES (?1, ?2, ?3)
       ON CONFLICT (account_id) DO UPDATE SET since = excluded.since, resets_at = excluded.resets_at",
      params![
        account.to_string(),
        hold.since.unix_millis(),
        hold.resets.map(Timestamp::unix_millis)
      ],
    )?;
    Ok(())
  }

  /// Forgets that `account` was at its plan limit; nothing when it was not.
  pub fn release_orchestrator(&mut self, account: AgentAccountId) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM orchestrator_plan_holds WHERE account_id = ?1",
      [account.to_string()],
    )?;
    Ok(())
  }

  /// The plan limit kept of `account`; `None` while none is.
  pub fn orchestrator_hold(&self, account: AgentAccountId) -> Result<Option<PlanHold>, StoreError> {
    let hold = self
      .connection
      .query_row(
        "SELECT since, resets_at FROM orchestrator_plan_holds WHERE account_id = ?1",
        [account.to_string()],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
      )
      .optional()?;
    Ok(hold.map(|(since, resets)| PlanHold {
      since: Timestamp::from_unix_millis(since),
      resets: resets.map(Timestamp::from_unix_millis),
    }))
  }
}
