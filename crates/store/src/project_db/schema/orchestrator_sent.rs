//! Where a message or an answer the Orchestrator chat wrote came from and how it went out (ORC-45): the chat's
//! conversation, and whether the chat sent it itself (`direct`) or the person applied its proposal
//! (`applied_by_person`). Both stay NULL for a row the chat did not write, and for one kept before they were.

pub(super) const MIGRATION: &str = r"
ALTER TABLE messages ADD COLUMN orchestrator_chat TEXT;
ALTER TABLE messages ADD COLUMN orchestrator_way TEXT
  CHECK (orchestrator_way IS NULL OR orchestrator_way IN ('direct', 'applied_by_person'));
ALTER TABLE questions ADD COLUMN orchestrator_chat TEXT;
ALTER TABLE questions ADD COLUMN orchestrator_way TEXT
  CHECK (orchestrator_way IS NULL OR orchestrator_way IN ('direct', 'applied_by_person'));
";
