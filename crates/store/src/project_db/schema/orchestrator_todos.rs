//! The todo list each turn of the Orchestrator chat keeps (`orchester_core::connection::stream::TodoList`): the
//! latest update of its agent replaces the one before. A step execution's list is read from its transcript instead.

pub(super) const MIGRATION: &str = r"
-- One row per turn, by the message that started it, once its agent wrote a list: the list as JSON, never empty.
CREATE TABLE orchestrator_todos (
  message_id TEXT PRIMARY KEY REFERENCES orchestrator_entries (message_id) ON DELETE CASCADE,
  items TEXT NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;
";
