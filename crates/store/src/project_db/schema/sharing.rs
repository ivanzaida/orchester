//! Shared projects (`docs/architecture/server.md`, "Desktop data"): every task gets the revision
//! each stored change raises; a shared project's tasks are the cache of the server's, written by the
//! desktop from what the server confirmed, with the pipeline by slug, the former key and who created
//! and last changed them. `sharing` holds the link to the server project (no row: the project is
//! local), `task_claims` and `task_run_status` the cached claim and run status of each task, and
//! `shared_automatic_status` the workflow status a run of a shared task moved it to and from, which
//! the holder's desktop sends to the server as Orchester's change while the server still has the
//! status it changed from.

pub(super) const MIGRATION: &str = r"
ALTER TABLE tasks ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0);
ALTER TABLE tasks ADD COLUMN sharing TEXT CHECK (sharing IN ('shared', 'not_shared'));
ALTER TABLE tasks ADD COLUMN pipeline_slug TEXT;
ALTER TABLE tasks ADD COLUMN former_key TEXT;
ALTER TABLE tasks ADD COLUMN created_by TEXT;
ALTER TABLE tasks ADD COLUMN updated_by TEXT;
ALTER TABLE tasks ADD COLUMN updated_at INTEGER;

CREATE TABLE sharing (
  only_row INTEGER PRIMARY KEY CHECK (only_row = 1),
  server_id TEXT NOT NULL,
  project_id TEXT NOT NULL,
  project_key TEXT NOT NULL,
  role TEXT CHECK (role IN ('owner', 'member')),
  joined_at INTEGER NOT NULL,
  event_cursor INTEGER NOT NULL CHECK (event_cursor >= 0),
  last_synced_at INTEGER NOT NULL,
  -- When the server answered that this person is no longer a member; NULL while they are.
  removed_at INTEGER
) STRICT;

CREATE TABLE task_claims (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id),
  claim_id TEXT NOT NULL,
  holder_id TEXT NOT NULL,
  holder_name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('manual', 'run')),
  since INTEGER NOT NULL,
  heartbeat_at INTEGER NOT NULL,
  stale INTEGER NOT NULL CHECK (stale IN (0, 1)),
  taken_over_from_id TEXT,
  taken_over_from_name TEXT
) STRICT;

CREATE TABLE task_run_status (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id),
  holder_id TEXT NOT NULL,
  holder_name TEXT NOT NULL,
  number INTEGER NOT NULL CHECK (number > 0),
  phase TEXT NOT NULL,
  step TEXT,
  started_at INTEGER NOT NULL,
  phase_since INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;

CREATE TABLE shared_automatic_status (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id),
  -- The status the cache had when the run changed it: sent only while the server still has it.
  from_status TEXT NOT NULL,
  status TEXT NOT NULL,
  changed_at INTEGER NOT NULL,
  -- Why it was not sent ('changed': the server's status changed, to server_status; 'not_holder'),
  -- shown on the task until the person dismisses it; NULL while it waits to be sent.
  not_sent TEXT CHECK (not_sent IN ('changed', 'not_holder')),
  server_status TEXT
) STRICT;
";
