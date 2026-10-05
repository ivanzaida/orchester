//! Joining a shared repository (`docs/architecture/server.md`, "Two people share one repository").
//! The tasks a person keeps out of the server project stay here as *Not shared* with the key they
//! had (`kept_key`, the project key before joining; their number stays) and their tag names
//! (`kept_tag_names`, a JSON array): the project's tags become the server project's. Task numbers
//! are therefore unique only among the other tasks, and `tasks` is rebuilt, as SQLite cannot drop
//! a column's `UNIQUE` in place (foreign keys are enforced again after migrating).
//! `joined_keys` holds the keys the local tasks had when the project joined, for the mapping that
//! stays in Sharing until *Make local*: their task, the key before and the number of the run a
//! task was claimed for, when one had not ended.

pub(super) const MIGRATION: &str = r"
CREATE TABLE tasks_joining (
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
  CHECK ((budget_tokens IS NULL) = (max_duration_ms IS NULL)),
  CHECK ((sharing IS 'not_shared') = (kept_key IS NOT NULL)),
  CHECK ((sharing IS 'not_shared') = (kept_tag_names IS NOT NULL))
) STRICT;
INSERT INTO tasks_joining (id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status,
  created_at, pipeline_version, revision, sharing, pipeline_slug, former_key, created_by, updated_by, updated_at)
  SELECT id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status, created_at,
    pipeline_version, revision, sharing, pipeline_slug, former_key, created_by, updated_by, updated_at
  FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_joining RENAME TO tasks;
CREATE INDEX tasks_by_batch ON tasks (batch_key);
CREATE UNIQUE INDEX tasks_by_number ON tasks (number) WHERE sharing IS NOT 'not_shared';

CREATE TABLE joined_keys (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id),
  former_key TEXT NOT NULL,
  claimed_run INTEGER CHECK (claimed_run > 0)
) STRICT;
";
