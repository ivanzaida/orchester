//! Budgets became tokens instead of money (user decision 2026-09-25).

pub(super) const MIGRATION: &str = r"
-- Budgets are tokens, not money (user decision 2026-09-25). Money has no token equivalent, so
-- every task and run that had a money budget gets the default run budget of Project settings ›
-- Limits, 1,400,000 tokens, and keeps its time limit; a run without one gets the default 30
-- minutes. Nothing is lost: each former money amount stays in `money_budgets_before_tokens`.
-- Runs start from the tokens their invocations reported so far. Their logs from before this
-- migration have no usage events; their creation events are read with the converted budget.
-- Tables are rebuilt (foreign keys are enforced again after migrating), as SQLite cannot drop
-- the money columns' constraints in place.
CREATE TABLE money_budgets_before_tokens (
  owner TEXT NOT NULL CHECK (owner IN ('task', 'run')),
  owner_id TEXT NOT NULL,
  micros INTEGER NOT NULL,
  currency TEXT NOT NULL,
  PRIMARY KEY (owner, owner_id)
) STRICT;
INSERT INTO money_budgets_before_tokens (owner, owner_id, micros, currency)
  SELECT 'task', id, budget_micros, budget_currency FROM tasks WHERE budget_micros IS NOT NULL;
INSERT INTO money_budgets_before_tokens (owner, owner_id, micros, currency)
  SELECT 'run', id, budget_micros, currency FROM runs;

CREATE TABLE tasks_tokens (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  batch_key TEXT NOT NULL,
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  pipeline_id TEXT,
  budget_tokens INTEGER CHECK (budget_tokens > 0),
  max_duration_ms INTEGER CHECK (max_duration_ms > 0),
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  CHECK ((budget_tokens IS NULL) = (max_duration_ms IS NULL))
) STRICT;
INSERT INTO tasks_tokens (id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status,
  created_at)
  SELECT id, number, batch_key, title, brief, pipeline_id,
    CASE WHEN budget_micros IS NULL THEN NULL ELSE 1400000 END, max_duration_ms, status, created_at
  FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_tokens RENAME TO tasks;
CREATE INDEX tasks_by_batch ON tasks (batch_key);

CREATE TABLE runs_tokens (
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
    CHECK (status IN ('pending', 'running', 'paused', 'interrupted', 'succeeded', 'failed', 'cancelled')),
  status_reason TEXT CHECK ((status IN ('paused', 'interrupted', 'failed')) = (status_reason IS NOT NULL)),
  failed_step TEXT CHECK ((status = 'failed') = (failed_step IS NOT NULL)),
  last_sequence INTEGER NOT NULL CHECK (last_sequence > 0),
  created_at INTEGER NOT NULL,
  CHECK (pause_requested = 0 OR status = 'running')
) STRICT;
INSERT INTO runs_tokens (id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch, base_revision,
  initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested, status,
  status_reason, failed_step, last_sequence, created_at)
  SELECT runs.id, runs.number, runs.idempotency_key, runs.task_id, runs.pipeline_version_id, runs.snapshot, runs.branch,
    runs.base_revision, 1400000, COALESCE(tasks.max_duration_ms, 1800000), 1400000,
    COALESCE(tasks.max_duration_ms, 1800000),
    COALESCE((SELECT SUM(MAX(input_tokens - cached_input_tokens, 0) + output_tokens) FROM invocations
      WHERE invocations.run_id = runs.id), 0),
    0, runs.status, CASE runs.status WHEN 'paused' THEN 'requested' ELSE runs.status_reason END, runs.failed_step,
    runs.last_sequence, runs.created_at
  FROM runs LEFT JOIN tasks ON tasks.id = runs.task_id;
DROP TABLE runs;
ALTER TABLE runs_tokens RENAME TO runs;
CREATE INDEX runs_by_task ON runs (task_id, number);
CREATE INDEX runs_by_status ON runs (status, number);

-- Project settings › Limits; without a row the project uses the defaults.
CREATE TABLE project_limits (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  max_run_budget INTEGER NOT NULL CHECK (max_run_budget > 0),
  default_run_budget INTEGER NOT NULL CHECK (default_run_budget > 0 AND default_run_budget <= max_run_budget),
  daily_budget INTEGER NOT NULL CHECK (daily_budget > 0),
  default_time_limit_ms INTEGER NOT NULL CHECK (default_time_limit_ms > 0)
) STRICT;

-- The project's token usage per day is read from the turns of every transcript.
CREATE INDEX agent_events_by_kind ON agent_events (kind, at);
";
