//! Immutable task details captured for a run before later edits.

pub(super) const MIGRATION: &str = r"
-- A run's task prompt content stays fixed when the task is edited for future runs.
-- Existing runs receive a snapshot of their current task content on the first subsequent edit.
CREATE TABLE run_task_snapshots (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  criteria TEXT NOT NULL,
  pipeline_id TEXT,
  budget_tokens INTEGER,
  max_duration_ms INTEGER,
  CHECK ((budget_tokens IS NULL) = (max_duration_ms IS NULL))
) STRICT;
CREATE TRIGGER run_task_snapshots_no_update BEFORE UPDATE ON run_task_snapshots
BEGIN SELECT RAISE(ABORT, 'a run task snapshot is immutable'); END;
CREATE TRIGGER run_task_snapshots_no_delete BEFORE DELETE ON run_task_snapshots
BEGIN SELECT RAISE(ABORT, 'a run task snapshot is immutable'); END;
";
