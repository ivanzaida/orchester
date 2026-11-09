//! Flaky tests and quarantine (ORC-259, `orchester_core::test_step::flaky`): every flaky outcome a Test step saw,
//! append-only, and the tests the person quarantined, which still run but never fail a Test step.

pub(super) const MIGRATION: &str = r"
CREATE TABLE flaky_outcomes (
  id INTEGER PRIMARY KEY,
  suite TEXT NOT NULL,
  test TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('flaky', 'quarantined_failure')),
  run_id TEXT NOT NULL REFERENCES runs (id),
  at INTEGER NOT NULL
) STRICT;
CREATE INDEX flaky_outcomes_by_test ON flaky_outcomes (suite, test, at);
CREATE TRIGGER flaky_outcomes_no_update BEFORE UPDATE ON flaky_outcomes
BEGIN SELECT RAISE(ABORT, 'flaky outcomes are append-only'); END;
CREATE TRIGGER flaky_outcomes_no_delete BEFORE DELETE ON flaky_outcomes
BEGIN SELECT RAISE(ABORT, 'flaky outcomes are append-only'); END;

CREATE TABLE quarantined_tests (
  suite TEXT NOT NULL,
  test TEXT NOT NULL,
  since INTEGER NOT NULL,
  PRIMARY KEY (suite, test)
) STRICT;
";
