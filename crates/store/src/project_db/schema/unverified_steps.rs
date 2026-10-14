//! A step that could not check everything it is for reports `unverified`, with what and why
//! (`step_outputs.unverified`, a JSON list), and fails `unverified`; a step that was not run again on the revision
//! its latest failed or unverified result was for fails `unchanged`. A failed execution keeps the revision of the
//! run's branch it started on (`judged_revision`), which that rule compares. A person accepts such a step as it is:
//! the attempt after it runs the steps that had not passed (`accepted`, its step in `return_to`). SQLite cannot
//! change a CHECK constraint, so the three tables are rebuilt, keeping their `rowid`, indexes and triggers.

pub(super) const MIGRATION: &str = r"
CREATE TABLE attempts_accepted (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  number INTEGER NOT NULL CHECK (number > 0),
  reason TEXT NOT NULL
    CHECK (reason IN ('initial', 'retry', 'follow_up', 'recovery', 'restart', 'returned', 'accepted')),
  failed_step TEXT,
  return_to TEXT,
  note TEXT CHECK ((reason = 'returned') = (note IS NOT NULL)),
  status TEXT NOT NULL CHECK (status IN ('active', 'routed_back', 'succeeded', 'failed', 'cancelled')),
  UNIQUE (run_id, number),
  CHECK ((reason IN ('retry', 'returned')) = (failed_step IS NOT NULL)),
  CHECK ((reason = 'initial') = (return_to IS NULL))
) STRICT;
INSERT INTO attempts_accepted (rowid, id, run_id, number, reason, failed_step, return_to, note, status)
  SELECT rowid, id, run_id, number, reason, failed_step, return_to, note, status FROM attempts;
DROP TABLE attempts;
ALTER TABLE attempts_accepted RENAME TO attempts;

CREATE TABLE step_executions_unverified (
  id TEXT PRIMARY KEY,
  attempt_id TEXT NOT NULL REFERENCES attempts (id),
  step_key TEXT NOT NULL,
  profile_version_id TEXT,
  model TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'waiting', 'passed', 'failed', 'skipped')),
  wait_reason TEXT CHECK (wait_reason IN ('approval', 'plan', 'message')),
  failure TEXT CHECK (failure IN ('agent_failed', 'reported_failed', 'not_started', 'step_budget', 'merge_conflict',
    'checkpoint_failed', 'command_failed', 'unverified', 'unchanged')),
  judged_revision TEXT CHECK (judged_revision IS NULL OR (status = 'failed' AND length(judged_revision) = 40)),
  UNIQUE (attempt_id, step_key),
  CHECK ((status = 'waiting') = (wait_reason IS NOT NULL)),
  CHECK ((profile_version_id IS NULL) = (model IS NULL))
) STRICT;
INSERT INTO step_executions_unverified
  (rowid, id, attempt_id, step_key, profile_version_id, model, status, wait_reason, failure)
  SELECT rowid, id, attempt_id, step_key, profile_version_id, model, status, wait_reason, failure
  FROM step_executions;
DROP TABLE step_executions;
ALTER TABLE step_executions_unverified RENAME TO step_executions;

CREATE TABLE step_outputs_unverified (
  step_execution_id TEXT PRIMARY KEY REFERENCES step_executions (id),
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_key TEXT NOT NULL,
  outcome TEXT CHECK (outcome IN ('done', 'failed', 'unverified')),
  text TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  unverified TEXT CHECK (unverified IS NULL OR outcome = 'unverified')
) STRICT;
INSERT INTO step_outputs_unverified (rowid, step_execution_id, run_id, step_key, outcome, text, created_at)
  SELECT rowid, step_execution_id, run_id, step_key, outcome, text, created_at FROM step_outputs;
DROP TABLE step_outputs;
ALTER TABLE step_outputs_unverified RENAME TO step_outputs;
CREATE INDEX step_outputs_by_run ON step_outputs (run_id, created_at);
CREATE TRIGGER step_outputs_no_update BEFORE UPDATE ON step_outputs
BEGIN SELECT RAISE(ABORT, 'step outputs are append-only'); END;
CREATE TRIGGER step_outputs_no_delete BEFORE DELETE ON step_outputs
BEGIN SELECT RAISE(ABORT, 'step outputs are append-only'); END;
";
