//! A person returns the work of a run that ended, succeeded or failed, to one of its agent steps with a note
//! (`returned_after_end`, its step in `return_to`, ORC-44); unlike `returned` no failed step sent it back. SQLite
//! cannot change a CHECK constraint, so the table is rebuilt, keeping its `rowid`.

pub(super) const MIGRATION: &str = r"
CREATE TABLE attempts_returned_after_end (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  number INTEGER NOT NULL CHECK (number > 0),
  reason TEXT NOT NULL CHECK (reason IN ('initial', 'retry', 'follow_up', 'recovery', 'restart', 'returned',
    'accepted', 'returned_after_end')),
  failed_step TEXT,
  return_to TEXT,
  note TEXT CHECK ((reason IN ('returned', 'returned_after_end')) = (note IS NOT NULL)),
  status TEXT NOT NULL CHECK (status IN ('active', 'routed_back', 'succeeded', 'failed', 'cancelled')),
  UNIQUE (run_id, number),
  CHECK ((reason IN ('retry', 'returned')) = (failed_step IS NOT NULL)),
  CHECK ((reason = 'initial') = (return_to IS NULL))
) STRICT;
INSERT INTO attempts_returned_after_end (rowid, id, run_id, number, reason, failed_step, return_to, note, status)
  SELECT rowid, id, run_id, number, reason, failed_step, return_to, note, status FROM attempts;
DROP TABLE attempts;
ALTER TABLE attempts_returned_after_end RENAME TO attempts;
";
