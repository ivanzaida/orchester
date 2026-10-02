//! A run whose step's retries ran out, or whose step broke again after a restart, waits for a person
//! (user decision 2026-10-02) instead of failing: status `waiting` names the step in `failed_step`
//! and why in `status_reason`. Attempts gain two reasons: `restart` runs a step that broke again
//! (`return_to`), `returned` runs from the step a person returned the work to, with their note.
//! SQLite cannot change a CHECK constraint, so both tables are rebuilt, keeping their `rowid`.
//! Runs that already failed with `retries_exhausted` stay failed.

pub(super) const MIGRATION: &str = r"
CREATE TABLE runs_waiting (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  idempotency_key TEXT NOT NULL UNIQUE,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  pipeline_version_id TEXT NOT NULL,
  snapshot TEXT NOT NULL,
  branch TEXT NOT NULL,
  base_revision TEXT NOT NULL CHECK (length(base_revision) = 40),
  initial_budget_tokens INTEGER NOT NULL CHECK (initial_budget_tokens > 0),
  initial_max_duration_ms INTEGER NOT NULL CHECK (initial_max_duration_ms > 0),
  budget_tokens INTEGER NOT NULL CHECK (budget_tokens > 0),
  max_duration_ms INTEGER NOT NULL CHECK (max_duration_ms > 0),
  used_tokens INTEGER NOT NULL CHECK (used_tokens >= 0),
  pause_requested INTEGER NOT NULL CHECK (pause_requested IN (0, 1)),
  status TEXT NOT NULL
    CHECK (status IN ('pending', 'running', 'paused', 'interrupted', 'waiting', 'succeeded', 'failed', 'cancelled')),
  status_reason TEXT CHECK ((status IN ('paused', 'interrupted', 'waiting', 'failed')) = (status_reason IS NOT NULL)),
  failed_step TEXT CHECK ((status IN ('waiting', 'failed')) = (failed_step IS NOT NULL)),
  last_sequence INTEGER NOT NULL CHECK (last_sequence > 0),
  created_at INTEGER NOT NULL,
  limit_execution TEXT CHECK ((status_reason IS 'step_budget') = (limit_execution IS NOT NULL)),
  base_branch TEXT,
  CHECK (pause_requested = 0 OR status = 'running')
) STRICT;
INSERT INTO runs_waiting (rowid, id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch,
  base_revision, initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens,
  pause_requested, status, status_reason, failed_step, last_sequence, created_at, limit_execution, base_branch)
  SELECT rowid, id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch, base_revision,
    initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested,
    status, status_reason, failed_step, last_sequence, created_at, limit_execution, base_branch
  FROM runs;
DROP TABLE runs;
ALTER TABLE runs_waiting RENAME TO runs;
CREATE INDEX runs_by_task ON runs (task_id, number);
CREATE INDEX runs_by_status ON runs (status, number);

CREATE TABLE attempts_returned (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  number INTEGER NOT NULL CHECK (number > 0),
  reason TEXT NOT NULL CHECK (reason IN ('initial', 'retry', 'follow_up', 'recovery', 'restart', 'returned')),
  failed_step TEXT,
  return_to TEXT,
  note TEXT CHECK ((reason = 'returned') = (note IS NOT NULL)),
  status TEXT NOT NULL CHECK (status IN ('active', 'routed_back', 'succeeded', 'failed', 'cancelled')),
  UNIQUE (run_id, number),
  CHECK ((reason IN ('retry', 'returned')) = (failed_step IS NOT NULL)),
  CHECK ((reason = 'initial') = (return_to IS NULL))
) STRICT;
INSERT INTO attempts_returned (rowid, id, run_id, number, reason, failed_step, return_to, status)
  SELECT rowid, id, run_id, number, reason, failed_step, return_to, status FROM attempts;
DROP TABLE attempts;
ALTER TABLE attempts_returned RENAME TO attempts;
";
