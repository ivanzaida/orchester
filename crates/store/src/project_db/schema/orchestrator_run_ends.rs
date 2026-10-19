//! Since when the Orchestrator chat follows the ends of its project's runs (`docs/design/orchestrator-toolset.md`,
//! decision 5): a run that ended before is history, not news, so a project that gains this follows only what ends
//! from then on. One row, set when the migration runs, in milliseconds since the Unix epoch.

pub(super) const MIGRATION: &str = r"
CREATE TABLE orchestrator_follow (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  run_ends_since INTEGER NOT NULL
) STRICT;

INSERT INTO orchestrator_follow (id, run_ends_since)
  VALUES (1, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
";
