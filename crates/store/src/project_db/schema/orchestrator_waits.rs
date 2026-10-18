//! What the Orchestrator chat follows of the project's agents and answers them with (ORC-42,
//! `docs/design/orchestrator-toolset.md`, decision 5). A conversation keeps one entry per wait of an agent (`wait`,
//! its JSON in `body`, unique by `wait_key`), which may own the Orchestrator turn it started by itself, as a person's
//! message owns its turn. A check cannot change in place, so `orchestrator_entries` is made again with its rows,
//! which keep their `seq`; nothing refers to it. A message on the person's side and the answers to a question with
//! suggested answers name the Orchestrator chat when it wrote and sent them.

pub(super) const MIGRATION: &str = r"
CREATE TABLE orchestrator_entries_waits (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id TEXT NOT NULL REFERENCES orchestrator_chats (id),
  kind TEXT NOT NULL CHECK (kind IN ('person', 'reply', 'action', 'event', 'wait')),
  at INTEGER NOT NULL,
  body TEXT NOT NULL CHECK (body <> ''),
  message_id TEXT UNIQUE,
  turn TEXT CHECK (turn IS NULL OR turn IN ('thinking', 'answered', 'stopped', 'failed')),
  ended_at INTEGER,
  failure TEXT,
  model TEXT,
  -- What a wait is (its kind, run and what it waits on), so each is kept once.
  wait_key TEXT UNIQUE,
  CHECK ((message_id IS NULL) = (turn IS NULL)),
  CHECK (kind <> 'person' OR message_id IS NOT NULL),
  CHECK (kind IN ('person', 'wait') OR message_id IS NULL),
  CHECK ((kind = 'wait') = (wait_key IS NOT NULL)),
  CHECK ((turn = 'failed') = (failure IS NOT NULL)),
  CHECK ((kind = 'reply') = (model IS NOT NULL))
) STRICT;

INSERT INTO orchestrator_entries_waits (seq, chat_id, kind, at, body, message_id, turn, ended_at, failure, model)
  SELECT seq, chat_id, kind, at, body, message_id, turn, ended_at, failure, model FROM orchestrator_entries;
DROP TABLE orchestrator_entries;
ALTER TABLE orchestrator_entries_waits RENAME TO orchestrator_entries;
CREATE INDEX orchestrator_entries_by_chat ON orchestrator_entries (chat_id, seq);

ALTER TABLE messages ADD COLUMN by_orchestrator INTEGER NOT NULL DEFAULT 0 CHECK (by_orchestrator IN (0, 1));
ALTER TABLE questions ADD COLUMN by_orchestrator INTEGER NOT NULL DEFAULT 0 CHECK (by_orchestrator IN (0, 1));
";
