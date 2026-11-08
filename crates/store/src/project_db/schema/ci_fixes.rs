//! Fix CI on a run (ORC-335, `orchester_core::pull_request::ci::fix`): the run's *Fix CI automatically* switch with its
//! limit and when it last changed, and each Fix CI sent or stopped at, append-only, for the Conversation's event lines
//! and the switch's count.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_ci_fix_settings (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  automatic INTEGER NOT NULL CHECK (automatic IN (0, 1)),
  fix_limit INTEGER NOT NULL CHECK (fix_limit IN (1, 2, 3, 5)),
  changed_at INTEGER NOT NULL
) STRICT;
CREATE TABLE run_ci_fix_events (
  id INTEGER PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  at INTEGER NOT NULL,
  pull_request INTEGER NOT NULL CHECK (pull_request > 0),
  failure TEXT NOT NULL,
  -- The failing checks' names, as a JSON array.
  failing TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('sent', 'stopped')),
  message_id TEXT,
  -- An automatic send's place under its limit; NULL for a send by hand.
  fix_count INTEGER CHECK (fix_count > 0),
  fix_limit INTEGER CHECK (fix_limit IN (1, 2, 3, 5)),
  CHECK ((kind = 'sent') = (message_id IS NOT NULL)),
  CHECK (kind = 'sent' OR (fix_count IS NULL AND fix_limit IS NOT NULL)),
  CHECK ((fix_count IS NULL) OR (fix_limit IS NOT NULL))
) STRICT;
CREATE INDEX run_ci_fix_events_run ON run_ci_fix_events (run_id, id);
CREATE TRIGGER run_ci_fix_events_no_update BEFORE UPDATE ON run_ci_fix_events
BEGIN SELECT RAISE(ABORT, 'run CI fix events are append-only'); END;
CREATE TRIGGER run_ci_fix_events_no_delete BEFORE DELETE ON run_ci_fix_events
BEGIN SELECT RAISE(ABORT, 'run CI fix events are append-only'); END;
";

/// A Fix CI whose message waits because the task's pipeline was not ready on the computer: its kind, `not_ready`, holds
/// the message and neither a count nor a limit. The table is rebuilt for its `CHECK`s; the old one's index and
/// triggers go with it.
pub(super) const NOT_READY_MIGRATION: &str = r"
ALTER TABLE run_ci_fix_events RENAME TO run_ci_fix_events_before_not_ready;
CREATE TABLE run_ci_fix_events (
  id INTEGER PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  at INTEGER NOT NULL,
  pull_request INTEGER NOT NULL CHECK (pull_request > 0),
  failure TEXT NOT NULL,
  -- The failing checks' names, as a JSON array.
  failing TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('sent', 'stopped', 'not_ready')),
  message_id TEXT,
  -- An automatic send's place under its limit; NULL for a send by hand.
  fix_count INTEGER CHECK (fix_count > 0),
  fix_limit INTEGER CHECK (fix_limit IN (1, 2, 3, 5)),
  CHECK ((kind = 'stopped') = (message_id IS NULL)),
  CHECK (kind <> 'stopped' OR (fix_count IS NULL AND fix_limit IS NOT NULL)),
  CHECK (kind <> 'not_ready' OR (fix_count IS NULL AND fix_limit IS NULL)),
  CHECK ((fix_count IS NULL) OR (fix_limit IS NOT NULL))
) STRICT;
INSERT INTO run_ci_fix_events (id, run_id, at, pull_request, failure, failing, kind, message_id, fix_count, fix_limit)
  SELECT id, run_id, at, pull_request, failure, failing, kind, message_id, fix_count, fix_limit
  FROM run_ci_fix_events_before_not_ready;
DROP TABLE run_ci_fix_events_before_not_ready;
CREATE INDEX run_ci_fix_events_run ON run_ci_fix_events (run_id, id);
CREATE TRIGGER run_ci_fix_events_no_update BEFORE UPDATE ON run_ci_fix_events
BEGIN SELECT RAISE(ABORT, 'run CI fix events are append-only'); END;
CREATE TRIGGER run_ci_fix_events_no_delete BEFORE DELETE ON run_ci_fix_events
BEGIN SELECT RAISE(ABORT, 'run CI fix events are append-only'); END;
";
