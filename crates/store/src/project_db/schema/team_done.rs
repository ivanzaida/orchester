//! The Done a merged team run asks for its task (OA-30). A local task is Done once its run's work is merged; a team
//! task's status is its team server's, so the merge of a team run here (into its module's branch, or the base for a
//! task in no module) records that its task is to be set Done, and the desktop that holds the claim sends it. One row
//! per merged team run, made with its merge: `pending` until the server answered, then `sent` once the server has the
//! task Done, or `not_sent` with why (the server's refusal code, or the status that stood), kept for the person.
//! Nothing here is a secret, and no record is deleted.

pub(super) const MIGRATION: &str = r"
CREATE TABLE team_merged_done (
  run_id TEXT PRIMARY KEY REFERENCES team_runs (run_id),
  team_task_id TEXT NOT NULL,
  merged_at INTEGER NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('pending', 'sent', 'not_sent')),
  reason TEXT CHECK ((state = 'not_sent') = (reason IS NOT NULL)),
  settled_at INTEGER CHECK ((state = 'pending') = (settled_at IS NULL))
) STRICT;
CREATE INDEX team_merged_done_pending ON team_merged_done (merged_at) WHERE state = 'pending';
CREATE TRIGGER team_merged_done_settled_once BEFORE UPDATE ON team_merged_done
WHEN NEW.run_id IS NOT OLD.run_id OR NEW.team_task_id IS NOT OLD.team_task_id OR NEW.merged_at IS NOT OLD.merged_at
  OR OLD.state <> 'pending'
BEGIN SELECT RAISE(ABORT, 'a merged team run asks for its Done once and is settled once'); END;
CREATE TRIGGER team_merged_done_no_delete BEFORE DELETE ON team_merged_done
BEGIN SELECT RAISE(ABORT, 'the Done of merged team runs is never deleted'); END;
";
