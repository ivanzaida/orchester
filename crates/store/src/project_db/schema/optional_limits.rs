//! Token budgets and time limits are optional and have no maximum (user decision 2026-10-09): a
//! task's budget and time limit are each set or not (NULL is no limit), so are a run's creation and
//! current limits and a step budget raised for a run, and Project settings › Limits loses its
//! maximum run budget while its default run budget and time limit may be empty. Every stored value
//! is kept; a task without limits, a draft before, now has no limit. SQLite cannot change a column's
//! `NOT NULL` or a `CHECK` in place, so the tables are rebuilt, keeping their `rowid` (foreign keys
//! are enforced again after migrating), with the indexes and triggers they had.

pub(super) const MIGRATION: &str = r"
CREATE TABLE tasks_optional_limits (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL CHECK (number > 0),
  batch_key TEXT NOT NULL,
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  pipeline_id TEXT,
  budget_tokens INTEGER CHECK (budget_tokens > 0),
  max_duration_ms INTEGER CHECK (max_duration_ms > 0),
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  pipeline_version INTEGER CHECK (pipeline_version > 0),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  sharing TEXT CHECK (sharing IN ('shared', 'not_shared')),
  pipeline_slug TEXT,
  former_key TEXT,
  created_by TEXT,
  updated_by TEXT,
  updated_at INTEGER,
  kept_key TEXT,
  kept_tag_names TEXT,
  CHECK ((sharing IS 'not_shared') = (kept_key IS NOT NULL)),
  CHECK ((sharing IS 'not_shared') = (kept_tag_names IS NOT NULL))
) STRICT;
INSERT INTO tasks_optional_limits (rowid, id, number, batch_key, title, brief, pipeline_id, budget_tokens,
  max_duration_ms, status, created_at, pipeline_version, revision, sharing, pipeline_slug, former_key, created_by,
  updated_by, updated_at, kept_key, kept_tag_names)
  SELECT rowid, id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status, created_at,
    pipeline_version, revision, sharing, pipeline_slug, former_key, created_by, updated_by, updated_at, kept_key,
    kept_tag_names
  FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_optional_limits RENAME TO tasks;
CREATE INDEX tasks_by_batch ON tasks (batch_key);
CREATE UNIQUE INDEX tasks_by_number ON tasks (number) WHERE sharing IS NOT 'not_shared';

CREATE TABLE run_task_snapshots_optional_limits (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  criteria TEXT NOT NULL,
  pipeline_id TEXT,
  budget_tokens INTEGER,
  max_duration_ms INTEGER
) STRICT;
INSERT INTO run_task_snapshots_optional_limits (rowid, run_id, title, brief, criteria, pipeline_id, budget_tokens,
  max_duration_ms)
  SELECT rowid, run_id, title, brief, criteria, pipeline_id, budget_tokens, max_duration_ms FROM run_task_snapshots;
DROP TABLE run_task_snapshots;
ALTER TABLE run_task_snapshots_optional_limits RENAME TO run_task_snapshots;
CREATE TRIGGER run_task_snapshots_no_update BEFORE UPDATE ON run_task_snapshots
BEGIN SELECT RAISE(ABORT, 'a run task snapshot is immutable'); END;
CREATE TRIGGER run_task_snapshots_no_delete BEFORE DELETE ON run_task_snapshots
BEGIN SELECT RAISE(ABORT, 'a run task snapshot is immutable'); END;

CREATE TABLE runs_optional_limits (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  idempotency_key TEXT NOT NULL UNIQUE,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  pipeline_version_id TEXT NOT NULL,
  snapshot TEXT NOT NULL,
  branch TEXT NOT NULL,
  base_revision TEXT NOT NULL CHECK (length(base_revision) = 40),
  initial_budget_tokens INTEGER CHECK (initial_budget_tokens > 0),
  initial_max_duration_ms INTEGER CHECK (initial_max_duration_ms > 0),
  budget_tokens INTEGER CHECK (budget_tokens > 0),
  max_duration_ms INTEGER CHECK (max_duration_ms > 0),
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
  auto_approve INTEGER CHECK (auto_approve IN (0, 1)),
  auto_mode INTEGER NOT NULL DEFAULT 0 CHECK (auto_mode IN (0, 1)),
  origin_url TEXT,
  CHECK (pause_requested = 0 OR status = 'running')
) STRICT;
INSERT INTO runs_optional_limits (rowid, id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch,
  base_revision, initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens,
  pause_requested, status, status_reason, failed_step, last_sequence, created_at, limit_execution, base_branch,
  auto_approve, auto_mode, origin_url)
  SELECT rowid, id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch, base_revision,
    initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested,
    status, status_reason, failed_step, last_sequence, created_at, limit_execution, base_branch, auto_approve,
    auto_mode, origin_url
  FROM runs;
DROP TABLE runs;
ALTER TABLE runs_optional_limits RENAME TO runs;
CREATE INDEX runs_by_task ON runs (task_id, number);
CREATE INDEX runs_by_status ON runs (status, number);

CREATE TABLE run_step_budgets_optional (
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_key TEXT NOT NULL,
  budget_tokens INTEGER CHECK (budget_tokens > 0),
  PRIMARY KEY (run_id, step_key)
) STRICT;
INSERT INTO run_step_budgets_optional (rowid, run_id, step_key, budget_tokens)
  SELECT rowid, run_id, step_key, budget_tokens FROM run_step_budgets;
DROP TABLE run_step_budgets;
ALTER TABLE run_step_budgets_optional RENAME TO run_step_budgets;

CREATE TABLE project_limits_optional (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  default_run_budget INTEGER CHECK (default_run_budget > 0),
  daily_budget INTEGER NOT NULL CHECK (daily_budget > 0),
  default_time_limit_ms INTEGER CHECK (default_time_limit_ms > 0),
  agent_slots INTEGER NOT NULL DEFAULT 4 CHECK (agent_slots BETWEEN 1 AND 16)
) STRICT;
INSERT INTO project_limits_optional (id, default_run_budget, daily_budget, default_time_limit_ms, agent_slots)
  SELECT id, default_run_budget, daily_budget, default_time_limit_ms, agent_slots FROM project_limits;
DROP TABLE project_limits;
ALTER TABLE project_limits_optional RENAME TO project_limits;
";
