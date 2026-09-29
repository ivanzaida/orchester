//! Persisted wait time and single-use Codex tool decisions.

pub(super) const ACTIVE_TIME_MIGRATION: &str = r"
-- ORC-41: time spent waiting for a live Codex tool decision is not active agent time.
ALTER TABLE invocations ADD COLUMN inactive_ms INTEGER NOT NULL DEFAULT 0 CHECK (inactive_ms >= 0);
ALTER TABLE invocations ADD COLUMN waiting_since INTEGER;
";

pub(super) const MIGRATION: &str = r"
-- One pending Codex App Server request per row. The JSON-RPC payload and arbitrary tool
-- arguments never enter the database; a resolved request cannot be decided a second time.
CREATE TABLE tool_approvals (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  invocation_id TEXT NOT NULL REFERENCES invocations (id),
  agent TEXT NOT NULL,
  server TEXT NOT NULL,
  action TEXT NOT NULL,
  target TEXT,
  requested_at INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN (
    'pending', 'dispatching_approval', 'dispatching_denial', 'approved', 'denied', 'interrupted'
  )),
  decided_at INTEGER CHECK ((state = 'pending') = (decided_at IS NULL))
) STRICT;
CREATE INDEX tool_approvals_pending ON tool_approvals (run_id, requested_at) WHERE state = 'pending';
CREATE TRIGGER tool_approvals_decided_once BEFORE UPDATE ON tool_approvals
WHEN OLD.state IN ('approved', 'denied', 'interrupted') OR NEW.id IS NOT OLD.id OR NEW.run_id IS NOT OLD.run_id
  OR NEW.step_execution_id IS NOT OLD.step_execution_id OR NEW.invocation_id IS NOT OLD.invocation_id
  OR NEW.agent IS NOT OLD.agent OR NEW.server IS NOT OLD.server OR NEW.action IS NOT OLD.action
  OR NEW.target IS NOT OLD.target OR NEW.requested_at IS NOT OLD.requested_at
BEGIN SELECT RAISE(ABORT, 'a tool approval keeps its request and is decided once'); END;
";
