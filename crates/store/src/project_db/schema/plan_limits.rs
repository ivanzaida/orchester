//! Plan limits history (ORC-72): what an agent CLI reported of its account's limits during a run.

pub(super) const MIGRATION: &str = r"
-- One report of a CLI's plan limits during a run, as it reported it (the windows, plan, credits and
-- block), written by the runner. Kept 90 days: storing a report removes those older than that.
CREATE TABLE plan_limit_snapshots (
  id INTEGER PRIMARY KEY,
  provider TEXT NOT NULL,
  at INTEGER NOT NULL,
  run_id TEXT NOT NULL REFERENCES runs (id),
  body TEXT NOT NULL
) STRICT;
CREATE INDEX plan_limit_snapshots_by_time ON plan_limit_snapshots (at);
";
