//! Distinguishes command metadata from an MCP server with the same name.

pub(super) const MIGRATION: &str = r"
ALTER TABLE tool_approvals ADD COLUMN command INTEGER NOT NULL DEFAULT 0 CHECK (command IN (0, 1));
ALTER TABLE tool_approvals ADD COLUMN scope TEXT;
CREATE TRIGGER command_approvals_metadata_immutable BEFORE UPDATE ON tool_approvals
WHEN NEW.command IS NOT OLD.command OR NEW.scope IS NOT OLD.scope
BEGIN SELECT RAISE(ABORT, 'approval scope cannot change'); END;
CREATE TABLE approval_interruptions (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id)
) STRICT;
";
