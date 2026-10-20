//! Plan limits history (`plan_limit_snapshots`): each report of a CLI's limits during a run, kept
//! 90 days.

use orchester_core::{
  account::AccountCli,
  connection::plan_limits::{LimitReading, PlanLimits, ReadingSource},
  id::{AgentAccountId, RunId, StepExecutionId},
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
          origin: ReadingSource::Turn,
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
    self.last_reading_where("run_id = ?1", &[&run.to_string() as &dyn rusqlite::ToSql])
  }

  /// What `run` stopped at when `execution` reached its account's plan limit: the last report of the limits of the
  /// account `execution`'s agent last started on (another step of the run may run on another account), or, for a
  /// step that waits before its agent starts, of `step_account`, the account its connection runs on now. `run`'s own
  /// reports come first; a step that never started has none, so the project's last report of the account counts.
  pub fn plan_stop_reading(
    &self,
    run: RunId,
    execution: StepExecutionId,
    step_account: Option<AgentAccountId>,
  ) -> Result<Option<LimitReading>, StoreError> {
    let Some(account) = self.execution_account(execution)?.or(step_account) else {
      return self.last_limit_reading(run);
    };
    let of_run = self.last_reading_where(
      "run_id = ?1 AND (account_id = ?2 OR (account_id IS NULL AND provider = ?3))",
      &[
        &run.to_string() as &dyn rusqlite::ToSql,
        &account.to_string(),
        &default_provider(account),
      ],
    )?;
    match of_run {
      Some(reading) => Ok(Some(reading)),
      None => self.last_account_reading(account),
    }
  }

  /// The last report of `account`'s limits in this project, from any run. One that names no account, from before
  /// accounts, counts as the Default of its own CLI only (`LimitReading::account_id`).
  pub fn last_account_reading(&self, account: AgentAccountId) -> Result<Option<LimitReading>, StoreError> {
    self.last_reading_where(
      "(account_id = ?1 OR (account_id IS NULL AND provider = ?2))",
      &[&account.to_string() as &dyn rusqlite::ToSql, &default_provider(account)],
    )
  }

  /// The last report among the snapshots `condition` keeps.
  fn last_reading_where(
    &self,
    condition: &str,
    values: &[&dyn rusqlite::ToSql],
  ) -> Result<Option<LimitReading>, StoreError> {
    let row = self
      .connection
      .query_row(
        &format!(
          "SELECT provider, at, body, account_id FROM plan_limit_snapshots WHERE {condition} \
           ORDER BY at DESC, id DESC LIMIT 1"
        ),
        values,
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

/// The provider whose readings without an account are `account`'s: its CLI's, when `account` is that CLI's Default;
/// `None` (no reading matches) for any other account.
fn default_provider(account: AgentAccountId) -> Option<&'static str> {
  AccountCli::ALL
    .into_iter()
    .find(|cli| cli.default_account() == account)
    .map(|cli| provider_text(cli.provider()))
}
