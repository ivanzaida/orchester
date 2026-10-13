//! Plan limits history (`plan_limit_snapshots`): each report of a CLI's limits during a run, kept
//! 90 days.

use orchester_core::{
  connection::plan_limits::{LimitReading, PlanLimits},
  id::RunId,
  run::RunNumber,
  time::Timestamp,
  usage::limits::{LimitSnapshot, SnapshotSource, retained_since},
};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb,
  connections::{parse_provider, provider_text},
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const SCHEMA: u32 = 1;
const WHAT: &str = "plan limits snapshot";

impl ProjectDb {
  /// Stores what `run`'s CLI reported of its limits, and removes the reports older than 90 days
  /// before it, in one transaction.
  pub fn save_limit_snapshot(&mut self, run: RunId, reading: &LimitReading) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO plan_limit_snapshots (provider, at, run_id, body, account_id) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        provider_text(reading.provider),
        reading.at.unix_millis(),
        run.to_string(),
        encode_json(WHAT, SCHEMA, &reading.limits)?,
        reading.account.map(|account| account.to_string()),
      ],
    )?;
    transaction.execute(
      "DELETE FROM plan_limit_snapshots WHERE at < ?1",
      [retained_since(reading.at).unix_millis()],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// The reports stored from `since` on, oldest first, each with the run that reported it.
  pub fn limit_snapshots(&self, since: Timestamp) -> Result<Vec<LimitSnapshot>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT snapshots.provider, snapshots.at, snapshots.body, runs.number, tasks.number, snapshots.account_id \
       FROM plan_limit_snapshots AS snapshots \
       LEFT JOIN runs ON runs.id = snapshots.run_id LEFT JOIN tasks ON tasks.id = runs.task_id \
       WHERE snapshots.at >= ?1 ORDER BY snapshots.at, snapshots.id",
    )?;
    let rows = statement
      .query_map([since.unix_millis()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, i64>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, Option<i64>>(3)?,
          row.get::<_, Option<i64>>(4)?,
          row.get::<_, Option<String>>(5)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let project = self.project();
    rows
      .into_iter()
      .map(|(provider, at, body, run, task, account)| {
        let source = match (run, task) {
          (Some(run), Some(task)) => Some(SnapshotSource {
            project: project.name().as_str().into(),
            task_key: format!("{}-{task}", project.key().as_str()).into(),
            run: run_number(run)?,
          }),
          _ => None,
        };
        Ok(LimitSnapshot {
          provider: parse_provider(&provider)?,
          account: account
            .map(|account| parse("plan limits account", &account))
            .transpose()?,
          at: Timestamp::from_unix_millis(at),
          limits: decode_json::<PlanLimits>(WHAT, SCHEMA, &body)?,
          source,
        })
      })
      .collect()
  }
}

pub(super) fn run_number(number: i64) -> Result<RunNumber, StoreError> {
  u32::try_from(number)
    .ok()
    .and_then(std::num::NonZeroU32::new)
    .map(RunNumber::new)
    .ok_or_else(|| corrupt("run number", number))
}

impl ProjectDb {
  /// The last report of `run`'s CLIs' limits, with the account it came from: what a run stopped at a plan limit
  /// waits for.
  pub fn last_limit_reading(&self, run: RunId) -> Result<Option<LimitReading>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT provider, at, body, account_id FROM plan_limit_snapshots WHERE run_id = ?1 \
         ORDER BY at DESC, id DESC LIMIT 1",
        [run.to_string()],
        |row| {
          Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
          ))
        },
      )
      .optional()?;
    row
      .map(|(provider, at, body, account)| {
        Ok(LimitReading {
          provider: parse_provider(&provider)?,
          account: account
            .map(|account| parse("plan limits account", &account))
            .transpose()?,
          at: Timestamp::from_unix_millis(at),
          limits: decode_json::<PlanLimits>(WHAT, SCHEMA, &body)?,
        })
      })
      .transpose()
  }
}
