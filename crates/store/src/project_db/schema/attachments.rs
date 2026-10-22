//! Files a person attached to a message (ORC-57, `orchester_core::attachment`): a row each, linked to a message of a
//! task's conversation or to a person's message in the Orchestrator chat, in the order they were attached. The copy
//! of the file is `<repo>/.orchester/attachments/<id>.<extension>`, never a path to the person's own file.

pub(super) const MIGRATION: &str = r"
-- An attachment lives as long as its message: a task's messages are never deleted; a chat message the runner never
-- took is forgotten with its attachments.
CREATE TABLE attachments (
  id TEXT PRIMARY KEY,
  message_id TEXT REFERENCES messages (id),
  chat_message_id TEXT,
  position INTEGER NOT NULL CHECK (position >= 0),
  name TEXT NOT NULL CHECK (name <> ''),
  media_type TEXT NOT NULL
    CHECK (media_type IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp', 'application/pdf', 'text/plain')),
  size INTEGER NOT NULL CHECK (size > 0),
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
  width INTEGER CHECK (width > 0),
  height INTEGER CHECK (height > 0),
  created_at INTEGER NOT NULL,
  CHECK ((message_id IS NULL) <> (chat_message_id IS NULL)),
  CHECK ((width IS NULL) = (height IS NULL))
) STRICT;
CREATE UNIQUE INDEX attachments_by_message ON attachments (message_id, position) WHERE message_id IS NOT NULL;
CREATE UNIQUE INDEX attachments_by_chat_message ON attachments (chat_message_id, position)
  WHERE chat_message_id IS NOT NULL;
CREATE TRIGGER attachments_kept BEFORE UPDATE ON attachments
BEGIN SELECT RAISE(ABORT, 'an attachment never changes'); END;
";
