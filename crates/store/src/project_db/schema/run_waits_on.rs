//! Which run waits on which task (`orchester_core::run::RunWaitsOn`, ORC-759): the waiting run, the task it waits on,
//! the branch that task's work is to be merged into, since when, and how the wait ended. A run waits on one task at a
//! time; its ended waits stay as its history.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_waits_on (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id TEXT NOT NULL REFERENCES runs (id),
  waited_task_id TEXT NOT NULL REFERENCES task_identities (id),
  base_branch TEXT NOT NULL CHECK (base_branch <> ''),
  since INTEGER NOT NULL,
  ended_at INTEGER,
  -- merged: the waited task's work was merged into base_branch; canceled: the waited task was canceled;
  -- closed_unmerged: it was done without that merge; run_ended: the waiting run ended first.
  ended_as TEXT CHECK (ended_as IN ('merged', 'canceled', 'closed_unmerged', 'run_ended')),
  CHECK ((ended_at IS NULL) = (ended_as IS NULL))
) STRICT;
CREATE UNIQUE INDEX run_waits_on_active_by_run ON run_waits_on (run_id) WHERE ended_at IS NULL;
CREATE INDEX run_waits_on_active_by_waited ON run_waits_on (waited_task_id) WHERE ended_at IS NULL;
CREATE TRIGGER run_waits_on_end_once BEFORE UPDATE ON run_waits_on
WHEN OLD.ended_at IS NOT NULL OR NEW.run_id IS NOT OLD.run_id OR NEW.waited_task_id IS NOT OLD.waited_task_id
  OR NEW.base_branch IS NOT OLD.base_branch OR NEW.since IS NOT OLD.since
BEGIN SELECT RAISE(ABORT, 'a run wait keeps what it waits on and ends once'); END;
";
