//! Flaky tests and quarantine (`docs/architecture/agent-testing.md`, "Flaky tests and quarantine";
//! [`orchester_core::test_step::flaky`]) as Project settings › Services › Flaky tests reads and changes them, and the
//! domain MCP tools do: every test a Test step saw flaky or quarantined, where each stands (watched, quarantined), and
//! the person's move of a test to quarantine and back. Every function blocks on SQLite.

use std::path::Path;

use orchester_core::{
  test_step::flaky::{FlakyTest, TestKey, flaky_tests},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

/// Why a test was not quarantined.
#[derive(Debug, thiserror::Error)]
pub enum QuarantineError {
  /// No Test step of the project saw it flaky: a name that is mistyped, or a test that fails every time.
  #[error("the test was never flaky in a Test step of this project")]
  NotFlaky,
  #[error(transparent)]
  Store(#[from] StoreError),
}

/// The project's flaky and quarantined tests at `now`: quarantined first, then watched, then the others.
pub fn read(root: &Path, now: Timestamp) -> Result<Vec<FlakyTest>, StoreError> {
  let db = ProjectDb::open(root)?;
  Ok(flaky_tests(&db.flaky_outcomes()?, &db.quarantined_tests()?, now))
}

/// Quarantines `test` from `now`, a test a Test step saw flaky; `false` when it was quarantined already. Its failures
/// no longer fail the Test steps that start from then on.
pub fn quarantine(root: &Path, test: &TestKey, now: Timestamp) -> Result<bool, QuarantineError> {
  let mut db = ProjectDb::open(root)?;
  if !db.flaky_outcomes()?.iter().any(|outcome| outcome.test == *test) {
    return Err(QuarantineError::NotFlaky);
  }
  Ok(db.quarantine_test(test, now)?)
}

/// Takes `test` out of quarantine; `false` when it was not quarantined. Its failures fail the Test steps that start
/// from then on again.
pub fn release(root: &Path, test: &TestKey) -> Result<bool, StoreError> {
  ProjectDb::open(root)?.release_test(test)
}
