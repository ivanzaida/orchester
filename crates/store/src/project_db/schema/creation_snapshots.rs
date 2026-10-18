//! Immutable task details as the task was created.

pub(super) const MIGRATION: &str = r"
-- What a task asked for when it was created, kept as it was through later edits. Tasks created before
-- this table have no row: their creation version was not recorded and is never reconstructed.
-- A local task names its pipeline by id; a shared task by slug, as its row in tasks does.
CREATE TABLE task_creation_snapshots (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id),
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  criteria TEXT NOT NULL,
  pipeline_id TEXT,
  pipeline_slug TEXT,
  budget_tokens INTEGER,
  max_duration_ms INTEGER,
  CHECK (pipeline_id IS NULL OR pipeline_slug IS NULL)
) STRICT;
CREATE TRIGGER task_creation_snapshots_no_update BEFORE UPDATE ON task_creation_snapshots
BEGIN SELECT RAISE(ABORT, 'a task creation snapshot is immutable'); END;
CREATE TRIGGER task_creation_snapshots_no_delete BEFORE DELETE ON task_creation_snapshots
BEGIN SELECT RAISE(ABORT, 'a task creation snapshot is immutable'); END;
";
