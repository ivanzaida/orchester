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

/// What was archived and by whom, kept with a stop recorded since the confirmation named it (ORC-838): the task page
/// says who archived the resource and opens it. A stop recorded before has none of the three.
pub(super) const ARCHIVED_MIGRATION: &str = r"
ALTER TABLE archive_stops ADD COLUMN resource_id TEXT;
ALTER TABLE archive_stops ADD COLUMN resource_kind TEXT;
ALTER TABLE archive_stops ADD COLUMN archived_by TEXT;
";
