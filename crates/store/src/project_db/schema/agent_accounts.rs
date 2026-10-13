//! Agent accounts (`docs/design/agent-accounts.md`): plan limit reports belong to the account the CLI ran on, an
//! agent invocation records the account it started on, so a finished run still names it, and a run can stop at its
//! account's plan limit (`plan_limit`, its stopped execution in `limit_execution`) and continue a step on another
//! connection for this run only. SQLite cannot change a `CHECK` in place, so `runs` is rebuilt as
//! `optional_limits` left it, keeping its `rowid` and indexes.

pub(super) const MIGRATION: &str = r"
-- NULL for a report from before accounts, which was the CLI's Default account's.
ALTER TABLE plan_limit_snapshots ADD COLUMN account_id TEXT;

-- The account an agent CLI invocation started on, written once before it starts. Its name and email stay in the
-- application database.
CREATE TABLE invocation_accounts (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  account_id TEXT NOT NULL
) STRICT;
CREATE TRIGGER invocation_accounts_immutable BEFORE UPDATE ON invocation_accounts
BEGIN SELECT RAISE(ABORT, 'an invocation keeps the account it started on'); END;

-- The connection a step of a run continues on, chosen by a person when its account reached its plan limit.
CREATE TABLE run_step_connections (
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_key TEXT NOT NULL,
  connection_id TEXT NOT NULL,
  PRIMARY KEY (run_id, step_key)
) STRICT;

CREATE TABLE runs_plan_limits (
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
  limit_execution TEXT
    CHECK ((status_reason IS 'step_budget' OR status_reason IS 'plan_limit') = (limit_execution IS NOT NULL)),
  base_branch TEXT,
  auto_approve INTEGER CHECK (auto_approve IN (0, 1)),
  auto_mode INTEGER NOT NULL DEFAULT 0 CHECK (auto_mode IN (0, 1)),
  origin_url TEXT,
  CHECK (pause_requested = 0 OR status = 'running')
) STRICT;
INSERT INTO runs_plan_limits (rowid, id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch,
  base_revision, initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens,
  pause_requested, status, status_reason, failed_step, last_sequence, created_at, limit_execution, base_branch,
  auto_approve, auto_mode, origin_url)
  SELECT rowid, id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch, base_revision,
    initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested,
    status, status_reason, failed_step, last_sequence, created_at, limit_execution, base_branch, auto_approve,
    auto_mode, origin_url
  FROM runs;
DROP TABLE runs;
ALTER TABLE runs_plan_limits RENAME TO runs;
CREATE INDEX runs_by_task ON runs (task_id, number);
CREATE INDEX runs_by_status ON runs (status, number);
";
