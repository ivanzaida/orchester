//! A run's permission events (ORC-319, `orchester_core::run::PermissionEvent`): what its permissions menu changed of
//! how the run answers its requests — approving everything, stopping it, revoking a rule — and automation's refused
//! asking to widen it, for its Conversation's event lines. Append-only; the run's switch to Manual stays in `runs`.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_permission_events (
  id INTEGER PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  at INTEGER NOT NULL,
  by TEXT NOT NULL CHECK (by IN ('person', 'automation')),
  event TEXT NOT NULL
) STRICT;
CREATE INDEX run_permission_events_run ON run_permission_events (run_id, id);
CREATE TRIGGER run_permission_events_no_update BEFORE UPDATE ON run_permission_events
BEGIN SELECT RAISE(ABORT, 'run permission events are append-only'); END;
CREATE TRIGGER run_permission_events_no_delete BEFORE DELETE ON run_permission_events
BEGIN SELECT RAISE(ABORT, 'run permission events are append-only'); END;
";
