//! A person's archive and deletion of an Orchestrator conversation (ORC-329, *Archive* and *Delete* in History).
//! What its turns spent stays counted toward the project's daily budget once it is deleted, so the usage no longer
//! refers to the conversation: SQLite cannot drop a reference in place, so `orchestrator_usage` is made again with its
//! rows, which keep their `id`.

pub(super) const MIGRATION: &str = r"
-- When a person archived it: History lists it under Archived instead, until they unarchive it; NULL while it is not.
ALTER TABLE orchestrator_chats ADD COLUMN archived_at INTEGER;

CREATE TABLE orchestrator_usage_kept (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  -- The conversation the turn was in, kept after a person deleted it.
  chat_id TEXT NOT NULL,
  message_id TEXT NOT NULL,
  at INTEGER NOT NULL,
  input_tokens INTEGER NOT NULL CHECK (input_tokens >= 0),
  cached_input_tokens INTEGER NOT NULL CHECK (cached_input_tokens >= 0),
  output_tokens INTEGER NOT NULL CHECK (output_tokens >= 0),
  account_id TEXT
) STRICT;
INSERT INTO orchestrator_usage_kept
  (id, chat_id, message_id, at, input_tokens, cached_input_tokens, output_tokens, account_id)
  SELECT id, chat_id, message_id, at, input_tokens, cached_input_tokens, output_tokens, account_id
  FROM orchestrator_usage;
DROP TABLE orchestrator_usage;
ALTER TABLE orchestrator_usage_kept RENAME TO orchestrator_usage;
CREATE INDEX orchestrator_usage_by_time ON orchestrator_usage (at);
CREATE INDEX orchestrator_usage_by_message ON orchestrator_usage (message_id);
";
