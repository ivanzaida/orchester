//! Explicit decision model comparisons; independent of live approval decisions and their budgets.
pub(super) const MIGRATION: &str = r"
CREATE TABLE decision_tests (
  id TEXT PRIMARY KEY,
  connection_id TEXT NOT NULL REFERENCES decision_connections(id) ON DELETE CASCADE,
  active INTEGER NOT NULL CHECK (active IN (0, 1)),
  stop_requested INTEGER NOT NULL DEFAULT 0 CHECK (stop_requested IN (0, 1)),
  report TEXT NOT NULL
) STRICT;
CREATE UNIQUE INDEX decision_test_one_active ON decision_tests(active) WHERE active = 1;
";
