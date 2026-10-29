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
