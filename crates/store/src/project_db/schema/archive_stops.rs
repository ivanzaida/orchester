//! The runs stopped because a resource they pin was archived on the team's server: the notification that tells the
//! person is read from them, and names the resource as it was called then.

pub(super) const MIGRATION: &str = r"
CREATE TABLE archive_stops (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  task_id TEXT NOT NULL REFERENCES tasks (id),
  resource_name TEXT NOT NULL,
  stopped_at INTEGER NOT NULL
) STRICT;
";
