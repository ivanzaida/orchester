//! Every merge attempt into a run's base branch (`orchester_core::integration::attempt`), landed or not: who asked,
//! from where, what came of it (`detail`, JSON) with why it did not land, and how each stage went (`stages`, JSON).
//! Append-only. An attempt that passed its preflight and has not ended has a row in `merge_attempts_open`, which its
//! record replaces; one left there when the app or the runner ended is recorded as interrupted.

pub(super) const MIGRATION: &str = r"
CREATE TABLE merge_attempts (
  run_id TEXT NOT NULL REFERENCES runs (id),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  task_id TEXT NOT NULL REFERENCES tasks (id),
  at INTEGER NOT NULL,
  asked_by TEXT NOT NULL CHECK (asked_by IN ('person', 'automation', 'merge_queue', 'retry')),
  retry INTEGER CHECK ((asked_by = 'retry') = (retry IS NOT NULL) AND (retry IS NULL OR retry > 0)),
  via TEXT NOT NULL CHECK (via IN ('task_page', 'merge_run', 'merge_queue')),
  outcome TEXT NOT NULL CHECK (outcome IN ('merged', 'refused', 'conflicts', 'not_recorded', 'git_stopped',
    'left_changed', 'interrupted', 'stopped', 'moved')),
  detail TEXT NOT NULL,
  cause TEXT,
  stages TEXT NOT NULL,
  PRIMARY KEY (run_id, sequence)
) STRICT;
CREATE INDEX merge_attempts_by_time ON merge_attempts (at);
CREATE TRIGGER merge_attempts_no_update BEFORE UPDATE ON merge_attempts
BEGIN SELECT RAISE(ABORT, 'merge attempts are append-only'); END;
CREATE TRIGGER merge_attempts_no_delete BEFORE DELETE ON merge_attempts
BEGIN SELECT RAISE(ABORT, 'merge attempts are append-only'); END;

CREATE TABLE merge_attempts_open (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  at INTEGER NOT NULL,
  asked_by TEXT NOT NULL CHECK (asked_by IN ('person', 'automation', 'merge_queue', 'retry')),
  retry INTEGER CHECK ((asked_by = 'retry') = (retry IS NOT NULL) AND (retry IS NULL OR retry > 0)),
  via TEXT NOT NULL CHECK (via IN ('task_page', 'merge_run', 'merge_queue')),
  stage TEXT NOT NULL
) STRICT;
";

/// What kind of cause ended an attempt (`orchester_core::integration::attempt::CauseKind`, JSON), so the attempt page
/// names it, with the OS's error number or how long Git wrote nothing; `NULL` for attempts recorded before it was kept,
/// and for those that landed or ended for Orchester's own reason.
pub(super) const CAUSE_KIND_MIGRATION: &str = r"
ALTER TABLE merge_attempts ADD COLUMN cause_kind TEXT;
";
