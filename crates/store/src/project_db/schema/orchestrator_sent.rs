//! The Orchestrator chat's conversation a message or an answer it wrote came from (ORC-45). How it went out is who
//! decided it: `by_automation` of a message, `answered_by` of an answer (automation when the chat sent it itself, the
//! person when they applied its proposal). NULL for a row the chat did not write, and for one kept before it was.

pub(super) const MIGRATION: &str = r"
ALTER TABLE messages ADD COLUMN orchestrator_chat TEXT;
ALTER TABLE questions ADD COLUMN orchestrator_chat TEXT;
";
