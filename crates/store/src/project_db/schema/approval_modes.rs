//! Approval modes (`orchester_core::run::EffectiveMode`, `docs/design/approval-modes.md`): the project's *Allow
//! Autopilot for agents in this project*, off without a row, and a run's switch to Manual for itself. They replace *New
//! runs use auto mode*, *With auto mode on, approve everything in each run* and each run's auto mode, which are read
//! here once: a project that had both settings on allows Autopilot, and there a run whose auto mode a person had
//! turned off stays out of it, switched to Manual. The reviewer's choice stays in `project_auto_mode`.

pub(super) const MIGRATION: &str = r"
CREATE TABLE project_approvals (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  allow_autopilot INTEGER NOT NULL CHECK (allow_autopilot IN (0, 1))
) STRICT;
INSERT INTO project_approvals (id, allow_autopilot)
  SELECT 1, 1 FROM project_auto_mode WHERE id = 1 AND new_runs = 1
    AND EXISTS (SELECT 1 FROM project_approve_everything WHERE id = 1);
ALTER TABLE runs ADD COLUMN manual INTEGER NOT NULL DEFAULT 0 CHECK (manual IN (0, 1));
UPDATE runs SET manual = 1
  WHERE auto_mode = 0 AND EXISTS (SELECT 1 FROM project_approvals WHERE id = 1 AND allow_autopilot = 1);
ALTER TABLE runs DROP COLUMN auto_mode;
ALTER TABLE project_auto_mode DROP COLUMN new_runs;
DROP TABLE project_approve_everything;
";

/// When a run was switched to Manual for itself and who switched it, for its Conversation's row; a run the migration
/// above switched has neither.
pub(super) const SWITCH_MIGRATION: &str = r"
ALTER TABLE runs ADD COLUMN manual_at INTEGER;
ALTER TABLE runs ADD COLUMN manual_by TEXT CHECK (manual_by IN ('person', 'automation'));
";

/// The project's *Approve mode* (`orchester_core::run::ApproveMode`, the person's decision of 2026-11-11): Manual
/// without a row or with `allow_autopilot` off, Autopilot with it on, Dangerous auto-approve with this column on as
/// well, which allows Autopilot too, so no row holds a pair that contradicts itself. Every project keeps the mode it
/// had: Manual or Autopilot.
pub(super) const DANGEROUS_MIGRATION: &str = r"
ALTER TABLE project_approvals ADD COLUMN dangerous_auto_approve INTEGER NOT NULL DEFAULT 0
  CHECK (dangerous_auto_approve IN (0, 1) AND (dangerous_auto_approve = 0 OR allow_autopilot = 1));
";
