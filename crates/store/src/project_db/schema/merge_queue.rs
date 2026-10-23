//! The project's merge queue (`orchester_core::merge_queue`): one row per run that joined it, with where it stands
//! (`state`, JSON) and since when, its place in the order, its re-syncs in a row, the last check on the merged result
//! that passed (`checked`, JSON) and when a person approved its merge; and what happened to each run on its way
//! (`merge_queue_events`, append-only). A run squashed into its base branch is recorded in `run_merges` as `squash`;
//! SQLite cannot change a CHECK constraint, so that table is rebuilt.

pub(super) const MIGRATION: &str = r"
CREATE TABLE merge_queue (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  task_id TEXT NOT NULL REFERENCES tasks (id),
  finished_at INTEGER NOT NULL,
  position INTEGER NOT NULL,
  state TEXT NOT NULL,
  since INTEGER NOT NULL,
  resyncs INTEGER NOT NULL CHECK (resyncs >= 0),
  checked TEXT,
  approved_at INTEGER
) STRICT;
CREATE INDEX merge_queue_by_position ON merge_queue (position);

CREATE TABLE merge_queue_events (
  run_id TEXT NOT NULL REFERENCES runs (id),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  at INTEGER NOT NULL,
  event TEXT NOT NULL,
  PRIMARY KEY (run_id, sequence)
) STRICT;
CREATE TRIGGER merge_queue_events_no_update BEFORE UPDATE ON merge_queue_events
BEGIN SELECT RAISE(ABORT, 'merge queue events are append-only'); END;
CREATE TRIGGER merge_queue_events_no_delete BEFORE DELETE ON merge_queue_events
BEGIN SELECT RAISE(ABORT, 'merge queue events are append-only'); END;

CREATE TABLE run_merges_new (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  into_branch TEXT NOT NULL,
  revision TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('fast_forward', 'merge_commit', 'already_merged', 'squash')),
  merged_at INTEGER NOT NULL
) STRICT;
INSERT INTO run_merges_new (rowid, run_id, into_branch, revision, kind, merged_at)
  SELECT rowid, run_id, into_branch, revision, kind, merged_at FROM run_merges;
DROP TABLE run_merges;
ALTER TABLE run_merges_new RENAME TO run_merges;
";
