//! Who decided a plan: the person in the app, or automation through Orchester's MCP server
//! (`decide_plan`). Decisions recorded before it was kept count as the person's, as the app showed
//! them. `plans_decided_once` refuses any change to a decided plan, so it is dropped while those rows
//! are filled in and created again as it was. A column added later cannot check other columns of the
//! rows it is added to, so triggers tie `decided_by` to `decision` as `decided_at` is tied: a decided
//! plan names who decided it, and a plan that waits names no one.

pub(super) const MIGRATION: &str = r"
DROP TRIGGER plans_decided_once;
ALTER TABLE plans ADD COLUMN decided_by TEXT CHECK (decided_by IN ('person', 'automation'));
UPDATE plans SET decided_by = 'person' WHERE decision IS NOT NULL;
CREATE TRIGGER plans_decided_once BEFORE UPDATE ON plans
WHEN OLD.decision IS NOT NULL OR NEW.text IS NOT OLD.text OR NEW.step_execution_id IS NOT OLD.step_execution_id
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'a plan keeps its text and is decided once'); END;
CREATE TRIGGER plans_decided_by_inserted BEFORE INSERT ON plans
WHEN (NEW.decision IS NULL) IS NOT (NEW.decided_by IS NULL)
BEGIN SELECT RAISE(ABORT, 'a decided plan names who decided it, and a waiting plan names no one'); END;
CREATE TRIGGER plans_decided_by_updated BEFORE UPDATE ON plans
WHEN (NEW.decision IS NULL) IS NOT (NEW.decided_by IS NULL)
BEGIN SELECT RAISE(ABORT, 'a decided plan names who decided it, and a waiting plan names no one'); END;
";
