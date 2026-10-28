//! A notice that the run's base branch moved which the runner merged itself (ORC-151,
//! `orchester_core::base_move::NoticeDelivery::Merged`): its merge commit, `merged_revision`, when, `delivered_at`, and
//! the execution that started on it, `execution_id`, without an invocation. SQLite cannot change a CHECK constraint, so
//! `base_notices` is rebuilt.

pub(super) const MIGRATION: &str = r"
CREATE TABLE base_notices_merged (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  task_id TEXT NOT NULL REFERENCES tasks (id),
  branch TEXT NOT NULL,
  revision TEXT NOT NULL CHECK (length(revision) = 40),
  body TEXT NOT NULL,
  noticed_at INTEGER NOT NULL,
  deliver_after INTEGER NOT NULL,
  execution_id TEXT,
  invocation_id TEXT,
  delivered_at INTEGER,
  merged_revision TEXT CHECK (merged_revision IS NULL OR length(merged_revision) = 40),
  CHECK ((execution_id IS NULL) = (delivered_at IS NULL)),
  CHECK (delivered_at IS NULL OR (invocation_id IS NULL) = (merged_revision IS NOT NULL)),
  CHECK (delivered_at IS NOT NULL OR (invocation_id IS NULL AND merged_revision IS NULL))
) STRICT;
INSERT INTO base_notices_merged (rowid, id, run_id, task_id, branch, revision, body, noticed_at, deliver_after,
    execution_id, invocation_id, delivered_at)
  SELECT rowid, id, run_id, task_id, branch, revision, body, noticed_at, deliver_after, execution_id, invocation_id,
    delivered_at FROM base_notices;
DROP TABLE base_notices;
ALTER TABLE base_notices_merged RENAME TO base_notices;
CREATE UNIQUE INDEX base_notices_pending ON base_notices (run_id) WHERE delivered_at IS NULL;
CREATE INDEX base_notices_by_task ON base_notices (task_id, noticed_at);
";
