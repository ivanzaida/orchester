//! Project database migrations. Append only: never edit a migration that has shipped.

pub(super) const MIGRATIONS: &[&str] = &[
  r"
CREATE TABLE project (
  id TEXT PRIMARY KEY,
  key TEXT NOT NULL,
  name TEXT NOT NULL
) STRICT;

CREATE TABLE counters (
  name TEXT PRIMARY KEY,
  next INTEGER NOT NULL CHECK (next > 0)
) STRICT;
INSERT INTO counters (name, next) VALUES ('task', 1);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  batch_key TEXT NOT NULL,
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  pipeline_id TEXT,
  budget_micros INTEGER,
  budget_currency TEXT,
  max_duration_ms INTEGER,
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  CHECK ((budget_micros IS NULL) = (budget_currency IS NULL)),
  CHECK ((budget_micros IS NULL) = (max_duration_ms IS NULL))
) STRICT;
CREATE INDEX tasks_by_batch ON tasks (batch_key);

CREATE TABLE task_criteria (
  task_id TEXT NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  text TEXT NOT NULL,
  PRIMARY KEY (task_id, position)
) STRICT;
",
  r"
CREATE TABLE resources (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('agent_profile', 'instruction', 'pipeline', 'tool')),
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (kind, slug)
) STRICT;

CREATE TABLE resource_drafts (
  resource_id TEXT PRIMARY KEY REFERENCES resources (id),
  body TEXT NOT NULL,
  based_on INTEGER CHECK (based_on > 0),
  updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE resource_versions (
  id TEXT PRIMARY KEY,
  resource_id TEXT NOT NULL REFERENCES resources (id),
  number INTEGER NOT NULL CHECK (number > 0),
  body TEXT NOT NULL,
  published_at INTEGER NOT NULL,
  UNIQUE (resource_id, number)
) STRICT;
CREATE TRIGGER resource_versions_no_update BEFORE UPDATE ON resource_versions
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
CREATE TRIGGER resource_versions_no_delete BEFORE DELETE ON resource_versions
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
",
  r"
INSERT INTO counters (name, next) VALUES ('run', 1);

CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE CHECK (number > 0),
  idempotency_key TEXT NOT NULL UNIQUE,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  pipeline_version_id TEXT NOT NULL,
  snapshot TEXT NOT NULL,
  branch TEXT NOT NULL,
  base_revision TEXT NOT NULL CHECK (length(base_revision) = 40),
  currency TEXT NOT NULL,
  budget_micros INTEGER NOT NULL CHECK (budget_micros > 0),
  spent_micros INTEGER NOT NULL CHECK (spent_micros >= 0),
  status TEXT NOT NULL
    CHECK (status IN ('pending', 'running', 'paused', 'interrupted', 'succeeded', 'failed', 'cancelled')),
  status_reason TEXT CHECK ((status IN ('interrupted', 'failed')) = (status_reason IS NOT NULL)),
  failed_step TEXT CHECK ((status = 'failed') = (failed_step IS NOT NULL)),
  last_sequence INTEGER NOT NULL CHECK (last_sequence > 0),
  created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX runs_by_task ON runs (task_id, number);
CREATE INDEX runs_by_status ON runs (status, number);

CREATE TABLE attempts (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  number INTEGER NOT NULL CHECK (number > 0),
  reason TEXT NOT NULL CHECK (reason IN ('initial', 'retry')),
  failed_step TEXT,
  return_to TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'routed_back', 'succeeded', 'failed', 'cancelled')),
  UNIQUE (run_id, number),
  CHECK ((reason = 'retry') = (failed_step IS NOT NULL)),
  CHECK ((reason = 'retry') = (return_to IS NOT NULL))
) STRICT;

CREATE TABLE step_executions (
  id TEXT PRIMARY KEY,
  attempt_id TEXT NOT NULL REFERENCES attempts (id),
  step_key TEXT NOT NULL,
  profile_version_id TEXT,
  model TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'waiting', 'passed', 'failed', 'skipped')),
  wait_reason TEXT CHECK (wait_reason IN ('approval', 'plan', 'message')),
  UNIQUE (attempt_id, step_key),
  CHECK ((status = 'waiting') = (wait_reason IS NOT NULL)),
  CHECK ((profile_version_id IS NULL) = (model IS NULL))
) STRICT;

CREATE TABLE checkpoints (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  revision TEXT NOT NULL CHECK (length(revision) = 40),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  created_at INTEGER NOT NULL,
  UNIQUE (run_id, sequence)
) STRICT;
CREATE TRIGGER checkpoints_no_update BEFORE UPDATE ON checkpoints
BEGIN SELECT RAISE(ABORT, 'checkpoints are immutable'); END;
CREATE TRIGGER checkpoints_no_delete BEFORE DELETE ON checkpoints
BEGIN SELECT RAISE(ABORT, 'checkpoints are immutable'); END;

CREATE TABLE run_events (
  run_id TEXT NOT NULL REFERENCES runs (id),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  payload TEXT NOT NULL,
  PRIMARY KEY (run_id, sequence)
) STRICT;
CREATE TRIGGER run_events_no_update BEFORE UPDATE ON run_events
BEGIN SELECT RAISE(ABORT, 'run events are append-only'); END;
CREATE TRIGGER run_events_no_delete BEFORE DELETE ON run_events
BEGIN SELECT RAISE(ABORT, 'run events are append-only'); END;
",
];
