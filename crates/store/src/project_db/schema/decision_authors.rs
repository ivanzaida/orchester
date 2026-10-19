//! Attribution of free-text answers and continuations, including messages from MCP clients.
pub(super) const MIGRATION: &str = r"
ALTER TABLE messages ADD COLUMN by_automation INTEGER NOT NULL DEFAULT 0 CHECK (by_automation IN (0, 1));
UPDATE messages SET by_automation = 1 WHERE by_orchestrator = 1;
";
