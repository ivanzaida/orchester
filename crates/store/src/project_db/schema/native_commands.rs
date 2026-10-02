//! What a native agent's command asks with besides the command (04.H9): the shell its string goes to,
//! and how long it may run. Both stay NULL for a CLI's request.

pub(super) const MIGRATION: &str = r"
ALTER TABLE tool_approvals ADD COLUMN native_shell TEXT
  CHECK (native_shell IN ('pwsh', 'windows_powershell', 'zsh'));
ALTER TABLE tool_approvals ADD COLUMN native_time_limit INTEGER CHECK (native_time_limit > 0);
CREATE TRIGGER native_command_approvals_valid BEFORE INSERT ON tool_approvals
WHEN NEW.native_shell IS NOT NULL AND NEW.native_time_limit IS NULL
BEGIN SELECT RAISE(ABORT, 'a native shell needs a native time limit'); END;
CREATE TRIGGER native_command_approvals_immutable BEFORE UPDATE ON tool_approvals
WHEN NEW.native_shell IS NOT OLD.native_shell OR NEW.native_time_limit IS NOT OLD.native_time_limit
BEGIN SELECT RAISE(ABORT, 'approval scope cannot change'); END;
";
