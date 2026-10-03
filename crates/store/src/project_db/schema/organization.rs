pub(super) const MIGRATION: &str = r"
CREATE TABLE task_organization (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE project_tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  normalized TEXT NOT NULL UNIQUE
) STRICT;
CREATE TABLE organization_changes (
  sequence INTEGER PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  actor TEXT NOT NULL,
  changed_at INTEGER NOT NULL,
  old_value TEXT NOT NULL,
  new_value TEXT NOT NULL,
  old_tag_names TEXT NOT NULL,
  new_tag_names TEXT NOT NULL
) STRICT;
CREATE INDEX organization_changes_by_task ON organization_changes(task_id, sequence);
-- Existing tasks start with the status their runs imply (user decision, 2026-10-03): a run that has
-- not ended (pending, running, paused, interrupted) is In progress; a latest run merged is Done; a
-- latest run ready for review (succeeded, not merged) is In review; a queued task without one of
-- those (no run yet, or a latest run that failed or was cancelled) is Todo; any other task stays
-- Backlog, which needs no row. No history row is written: nobody changed them.
INSERT INTO task_organization (task_id, body, updated_at)
SELECT id, json_object('schema', 1, 'body', json_object(
    'status', status, 'priority', 'None', 'blocked_reason', NULL,
    'dependencies', json('[]'), 'tags', json('[]'))), created_at
FROM (
  SELECT tasks.id, tasks.created_at, CASE
    WHEN EXISTS (SELECT 1 FROM runs WHERE runs.task_id = tasks.id
      AND runs.status NOT IN ('succeeded', 'failed', 'cancelled')) THEN 'InProgress'
    WHEN latest.status = 'succeeded'
      AND EXISTS (SELECT 1 FROM run_merges WHERE run_merges.run_id = latest.id) THEN 'Done'
    WHEN latest.status = 'succeeded' THEN 'InReview'
    WHEN tasks.status = 'queued' THEN 'Todo'
    ELSE 'Backlog'
  END AS status
  FROM tasks
  LEFT JOIN runs AS latest
    ON latest.id = (SELECT id FROM runs WHERE runs.task_id = tasks.id ORDER BY number DESC LIMIT 1)
)
WHERE status <> 'Backlog';
";
