//! The project's share of the computer's *Services at once* (ORC-248, `docs/architecture/agent-testing.md`, decision
//! 3): how many services its runs may run at once, saved with the project's other limits.

pub(super) const MIGRATION: &str = r"
-- Without a row the project uses the defaults; a row saved before this keeps the default share of 4.
ALTER TABLE project_limits ADD COLUMN service_slots INTEGER NOT NULL DEFAULT 4 CHECK (service_slots BETWEEN 1 AND 32);
";
