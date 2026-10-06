//! Auto mode (`orchester_core::run::review`): the project's setting for new runs and the reviewer's
//! model, each run's own choice, and what auto mode did with a request (`review`, JSON of
//! `Review`), with `decided_by = 'reviewer'` for a request its reviewer approved. SQLite cannot widen a
//! column's `CHECK` in place, so `tool_approvals` is rebuilt with every column, index and trigger it had
//! (foreign keys are enforced again after migrating).

pub(super) const MIGRATION: &str = r"
-- Whether new runs use auto mode, and the reviewer's model; a project without a row has it off.
CREATE TABLE project_auto_mode (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  new_runs INTEGER NOT NULL CHECK (new_runs IN (0, 1)),
  model TEXT NOT NULL
) STRICT;
-- A run's own auto mode, taken from the project's setting when the run is created.
ALTER TABLE runs ADD COLUMN auto_mode INTEGER NOT NULL DEFAULT 0 CHECK (auto_mode IN (0, 1));
CREATE TABLE tool_approvals_reviewed (
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
    'auto_approved', 'permissions', 'automation', 'automation_deny', 'reviewer'
  )),
  rule_id INTEGER,
  review TEXT
) STRICT;
INSERT INTO tool_approvals_reviewed (id, run_id, step_execution_id, invocation_id, agent, server, action, target,
  requested_at, state, decided_at, command, scope, native_shell, native_time_limit, decided_by, rule_id)
  SELECT id, run_id, step_execution_id, invocation_id, agent, server, action, target, requested_at, state,
    decided_at, command, scope, native_shell, native_time_limit, decided_by, rule_id
  FROM tool_approvals;
DROP TABLE tool_approvals;
ALTER TABLE tool_approvals_reviewed RENAME TO tool_approvals;
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

/// The reviewer a person chose: any agent connection of the project, a model of it, and the effort.
/// Without a connection the reviewer is the default one, and `model` is not read.
pub(super) const REVIEWER_MIGRATION: &str = r"
ALTER TABLE project_auto_mode ADD COLUMN connection TEXT;
ALTER TABLE project_auto_mode ADD COLUMN effort TEXT CHECK (effort IS NULL OR connection IS NOT NULL);
";

/// The push URL of `origin` when a run started, which a push auto mode reviews must still go to.
pub(super) const ORIGIN_MIGRATION: &str = r"
ALTER TABLE runs ADD COLUMN origin_url TEXT;
";
