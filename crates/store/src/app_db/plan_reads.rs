//! Plan limits Orchester read outside a run (`plan_limit_reads`): each read of an account's limits, kept 90 days
//! like the reports turns store in their project (`project_db::plan_limits`). A read belongs to the account, not to a
//! project, so it is kept here.

use orchester_core::{
  account::AccountCli,
  connection::plan_limits::{LimitReading, PlanLimits, ReadingSource},
  id::AgentAccountId,
  time::Timestamp,
  usage::limits::{LimitSnapshot, retained_since},
};
use rusqlite::{OptionalExtension, params};

use super::AppDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

pub(super) const MIGRATION: &str = r"
CREATE TABLE plan_limit_reads (
  id INTEGER PRIMARY KEY,
  account_id TEXT NOT NULL,
  cli TEXT NOT NULL CHECK (cli IN ('claude_code', 'codex')),
  at INTEGER NOT NULL,
  body TEXT NOT NULL
) STRICT;

CREATE INDEX plan_limit_reads_by_time ON plan_limit_reads (at);
CREATE INDEX plan_limit_reads_by_account ON plan_limit_reads (account_id, at);
";

const SCHEMA: u32 = 1;
const WHAT: &str = "plan limits read";

/// A row of `plan_limit_reads`: account, CLI, time and limits.
type ReadRow = (String, String, i64, String);

impl AppDb {
  /// Stores what a read at `at` found of the limits of `account`, an account of `cli`, and removes the reads older
  /// than 90 days before it, in one transaction.
  pub fn save_plan_read(
    &mut self,
    (account, cli): (AgentAccountId, AccountCli),
    at: Timestamp,
    limits: &PlanLimits,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO plan_limit_reads (account_id, cli, at, body) VALUES (?1, ?2, ?3, ?4)",
      params![
        account.to_string(),
        cli.as_str(),
        at.unix_millis(),
        encode_json(WHAT, SCHEMA, limits)?,
      ],
    )?;
    transaction.execute(
      "DELETE FROM plan_limit_reads WHERE at < ?1",
      [retained_since(at).unix_millis()],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// The reads stored from `since` on, oldest first, as snapshots of the history.
  pub fn plan_reads(&self, since: Timestamp) -> Result<Vec<LimitSnapshot>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT account_id, cli, at, body FROM plan_limit_reads WHERE at >= ?1 ORDER BY at, id")?;
    let rows = statement
      .query_map([since.unix_millis()], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
      })?
      .collect::<Result<Vec<ReadRow>, _>>()?;
    rows
      .into_iter()
      .map(|row| {
        let reading = reading(row)?;
        Ok(LimitSnapshot {
          provider: reading.provider,
          account: reading.account,
          at: reading.at,
          limits: reading.limits,
          source: None,
          origin: ReadingSource::Read,
        })
      })
      .collect()
  }

  /// The last read of `account`'s limits.
  pub fn last_plan_read(&self, account: AgentAccountId) -> Result<Option<LimitReading>, StoreError> {
    self
      .connection
      .query_row(
        "SELECT account_id, cli, at, body FROM plan_limit_reads WHERE account_id = ?1 ORDER BY at DESC, id DESC LIMIT 1",
        [account.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?
      .map(reading)
      .transpose()
  }
}

fn reading((account, cli, at, body): ReadRow) -> Result<LimitReading, StoreError> {
  let cli = AccountCli::parse(&cli).ok_or_else(|| corrupt("plan limits read CLI", &cli))?;
  Ok(LimitReading {
    provider: cli.provider(),
    account: Some(parse("plan limits read account", &account)?),
    at: Timestamp::from_unix_millis(at),
    limits: decode_json::<PlanLimits>(WHAT, SCHEMA, &body)?,
  })
}
