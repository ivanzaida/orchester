//! When the base branch moves (ORC-54, `orchester_core::base_move`): where each base branch was last seen, the
//! notices to runs that it moved, and the project's switch for them.

pub(super) const MIGRATION: &str = r"
-- Where each base branch of the project's runs was last seen in the user's checkout, and when the move that put it
-- there began (NULL before a move was seen): moves within two minutes of that are one. Desktop-written.
CREATE TABLE base_heads (
  branch TEXT PRIMARY KEY,
  head TEXT NOT NULL CHECK (length(head) = 40),
  moved_at INTEGER
) STRICT;

-- Notices to runs that their base branch moved to `revision`, which their branch lacks: what landed, as JSON, and
-- once delivered the invocation that started with it. The desktop writes a notice, replacing the run's pending one;
-- the runner records its delivery. At most one pending notice per run.
CREATE TABLE base_notices (
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
  CHECK ((execution_id IS NULL) = (delivered_at IS NULL)),
  CHECK ((invocation_id IS NULL) = (delivered_at IS NULL))
) STRICT;
CREATE UNIQUE INDEX base_notices_pending ON base_notices (run_id) WHERE delivered_at IS NULL;
CREATE INDEX base_notices_by_task ON base_notices (task_id, noticed_at);

-- Project settings › General › Tell working agents when the base branch moves, written when a person sets it;
-- without a row it is on.
CREATE TABLE project_base_notices (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  setting TEXT NOT NULL CHECK (setting IN ('on', 'off'))
) STRICT;
";
