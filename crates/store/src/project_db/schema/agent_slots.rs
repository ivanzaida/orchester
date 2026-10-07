//! Project settings › Limits › Agents working at once (ORC-80): how many agents the project's runner
//! lets work at once, saved with the project's other limits.

pub(super) const MIGRATION: &str = r"
-- Without a row the project uses the defaults; a row saved before this keeps the default of 4.
ALTER TABLE project_limits ADD COLUMN agent_slots INTEGER NOT NULL DEFAULT 4 CHECK (agent_slots BETWEEN 1 AND 16);
";
