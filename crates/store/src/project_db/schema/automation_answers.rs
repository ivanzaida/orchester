//! Automation answers waiting requests through Orchester's MCP server when a person allows it in the
//! app (`DecidedBy::Automation`, `DecidedBy::AutomationDeny`): it approves once or denies. SQLite cannot
//! widen a column's `CHECK` in place, so `tool_approvals` is rebuilt, after the `permissions` rebuild,
//! with every column, index and trigger it had (foreign keys are enforced again after migrating).

pub(super) const MIGRATION: &str = r"
CREATE TABLE tool_approvals_answered (
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
  decided_at INTEGER CHECK ((state = 'pending') = (decided_at IS NULL)),
  command INTEGER NOT NULL DEFAULT 0 CHECK (command IN (0, 1)),
  scope TEXT,
  native_shell TEXT CHECK (native_shell IN ('pwsh', 'windows_powershell', 'zsh')),
  native_time_limit INTEGER CHECK (native_time_limit > 0),
  decided_by TEXT CHECK (decided_by IN (
    'person', 'person_similar', 'person_always', 'person_run', 'person_deny', 'similar_rule', 'always_allowed',
    'auto_approved', 'permissions', 'automation', 'automation_deny'
  )),
  rule_id INTEGER
) STRICT;
INSERT INTO tool_approvals_answered (id, run_id, step_execution_id, invocation_id, agent, server, action, target,
  requested_at, state, decided_at, command, scope, native_shell, native_time_limit, decided_by, rule_id)
  SELECT id, run_id, step_execution_id, invocation_id, agent, server, action, target, requested_at, state,
    decided_at, command, scope, native_shell, native_time_limit, decided_by, rule_id
  FROM tool_approvals;
DROP TABLE tool_approvals;
ALTER TABLE tool_approvals_answered RENAME TO tool_approvals;
CREATE INDEX tool_approvals_pending ON tool_approvals (run_id, requested_at) WHERE state = 'pending';
CREATE TRIGGER tool_approvals_decided_once BEFORE UPDATE ON tool_approvals
WHEN OLD.state IN ('approved', 'denied', 'interrupted') OR NEW.id IS NOT OLD.id OR NEW.run_id IS NOT OLD.run_id
  OR NEW.step_execution_id IS NOT OLD.step_execution_id OR NEW.invocation_id IS NOT OLD.invocation_id
  OR NEW.agent IS NOT OLD.agent OR NEW.server IS NOT OLD.server OR NEW.action IS NOT OLD.action
  OR NEW.target IS NOT OLD.target OR NEW.requested_at IS NOT OLD.requested_at
BEGIN SELECT RAISE(ABORT, 'a tool approval keeps its request and is decided once'); END;
CREATE TRIGGER command_approvals_metadata_immutable BEFORE UPDATE ON tool_approvals
WHEN NEW.command IS NOT OLD.command OR NEW.scope IS NOT OLD.scope
BEGIN SELECT RAISE(ABORT, 'approval scope cannot change'); END;
CREATE TRIGGER native_command_approvals_valid BEFORE INSERT ON tool_approvals
WHEN NEW.native_shell IS NOT NULL AND NEW.native_time_limit IS NULL
BEGIN SELECT RAISE(ABORT, 'a native shell needs a native time limit'); END;
CREATE TRIGGER native_command_approvals_immutable BEFORE UPDATE ON tool_approvals
WHEN NEW.native_shell IS NOT OLD.native_shell OR NEW.native_time_limit IS NOT OLD.native_time_limit
BEGIN SELECT RAISE(ABORT, 'a native command keeps the shell and time limit it was requested with'); END;
";
