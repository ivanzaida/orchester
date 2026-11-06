//! Flaky tests and quarantine (`flaky_outcomes`, `quarantined_tests`, ORC-259): the runner appends each flaky outcome
//! its Test steps see and reads the quarantined tests when a Test step starts; the person quarantines a test and
//! releases it through the workspace.

use orchester_core::{
  resource::Slug,
  test_step::flaky::{FlakyKind, FlakyOutcome, TestKey},
  time::Timestamp,
};
use rusqlite::params;

use super::{ProjectDb, write_transaction::retried};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// Appends `outcomes` to the project's flaky history, all or none.
  pub fn record_flaky_outcomes(&mut self, outcomes: &[FlakyOutcome]) -> Result<(), StoreError> {
    if outcomes.is_empty() {
      return Ok(());
    }
    retried(&mut self.connection, "record flaky outcomes", |transaction| {
      let mut insert = transaction
        .prepare_cached("INSERT INTO flaky_outcomes (suite, test, kind, run_id, at) VALUES (?1, ?2, ?3, ?4, ?5)")?;
      for outcome in outcomes {
        insert.execute(params![
          outcome.test.suite().as_str(),
          outcome.test.test(),
          outcome.kind.as_str(),
          outcome.run.to_string(),
          outcome.at.unix_millis()
        ])?;
      }
      Ok(())
    })
  }

  /// The project's flaky history, oldest first.
  pub fn flaky_outcomes(&self) -> Result<Vec<FlakyOutcome>, StoreError> {
    let mut query = self
      .connection
      .prepare("SELECT suite, test, kind, run_id, at FROM flaky_outcomes ORDER BY id")?;
    let rows: Vec<(String, String, String, String, i64)> = query
      .query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
      })?
      .collect::<Result<_, _>>()?;
    rows
      .into_iter()
      .map(|(suite, test, kind, run, at)| {
        Ok(FlakyOutcome {
          test: key(&suite, &test)?,
          kind: FlakyKind::parse(&kind).ok_or_else(|| corrupt("flaky outcome kind", &kind))?,
          run: parse("flaky outcome run", &run)?,
          at: Timestamp::from_unix_millis(at),
        })
      })
      .collect()
  }

  /// The project's quarantined tests, each with when it was quarantined, by suite and name.
  pub fn quarantined_tests(&self) -> Result<Vec<(TestKey, Timestamp)>, StoreError> {
    let mut query = self
      .connection
      .prepare("SELECT suite, test, since FROM quarantined_tests ORDER BY suite, test")?;
    let rows: Vec<(String, String, i64)> = query
      .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
      .collect::<Result<_, _>>()?;
    rows
      .into_iter()
      .map(|(suite, test, since)| Ok((key(&suite, &test)?, Timestamp::from_unix_millis(since))))
      .collect()
  }

  /// Quarantines `test` from `at`; `false` when it was quarantined already, which keeps its first time.
  pub fn quarantine_test(&mut self, test: &TestKey, at: Timestamp) -> Result<bool, StoreError> {
    let added = self.connection.execute(
      "INSERT INTO quarantined_tests (suite, test, since) VALUES (?1, ?2, ?3) ON CONFLICT (suite, test) DO NOTHING",
      params![test.suite().as_str(), test.test(), at.unix_millis()],
    )?;
    Ok(added == 1)
  }

  /// Takes `test` out of quarantine; `false` when it was not quarantined.
  pub fn release_test(&mut self, test: &TestKey) -> Result<bool, StoreError> {
    let removed = self.connection.execute(
      "DELETE FROM quarantined_tests WHERE suite = ?1 AND test = ?2",
      params![test.suite().as_str(), test.test()],
    )?;
    Ok(removed == 1)
  }
}

fn key(suite: &str, test: &str) -> Result<TestKey, StoreError> {
  let suite = Slug::new(suite).map_err(|error| corrupt("flaky test suite", error))?;
  TestKey::new(suite, test).map_err(|error| corrupt("flaky test name", error))
}
