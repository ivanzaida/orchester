//! A person's title and pin of an Orchestrator conversation (ORC-328, *Rename* and *Pin* in History).

pub(super) const MIGRATION: &str = r"
-- The title a person gave the conversation, which wins over the one taken from its first message (`title`); NULL
-- while nobody renamed it.
ALTER TABLE orchestrator_chats ADD COLUMN person_title TEXT CHECK (person_title IS NULL OR trim(person_title) <> '');
-- When a person pinned it to History's top, which lists the latest pin first; NULL while it is not pinned.
ALTER TABLE orchestrator_chats ADD COLUMN pinned_at INTEGER;
";
