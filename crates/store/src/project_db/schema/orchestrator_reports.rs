//! The daily report of the project in the Orchestrator chat (ORC-47): a conversation keeps one entry per local day
//! (`report`, its JSON in `body`, unique by `wait_key`, `report:<day>`), which may own the Orchestrator turn it started
//! by itself to add a summary, as a wait does. A check cannot change in place, so `orchestrator_entries` is made again
//! with its rows, which keep their `seq`; nothing refers to it.

pub(super) const MIGRATION: &str = r"
CREATE TABLE orchestrator_entries_reports (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id TEXT NOT NULL REFERENCES orchestrator_chats (id),
  kind TEXT NOT NULL CHECK (kind IN ('person', 'reply', 'action', 'event', 'wait', 'report')),
  at INTEGER NOT NULL,
  body TEXT NOT NULL CHECK (body <> ''),
  message_id TEXT UNIQUE,
  turn TEXT CHECK (turn IS NULL OR turn IN ('thinking', 'answered', 'stopped', 'failed')),
  ended_at INTEGER,
  failure TEXT,
  model TEXT,
  -- What a wait is (its kind, run and what it waits on), or the day of a report, so each is kept once.
  wait_key TEXT UNIQUE,
  CHECK ((message_id IS NULL) = (turn IS NULL)),
  CHECK (kind <> 'person' OR message_id IS NOT NULL),
  CHECK (kind IN ('person', 'wait', 'report') OR message_id IS NULL),
  CHECK ((kind IN ('wait', 'report')) = (wait_key IS NOT NULL)),
  CHECK ((turn = 'failed') = (failure IS NOT NULL)),
  CHECK ((kind = 'reply') = (model IS NOT NULL))
) STRICT;

INSERT INTO orchestrator_entries_reports
  (seq, chat_id, kind, at, body, message_id, turn, ended_at, failure, model, wait_key)
  SELECT seq, chat_id, kind, at, body, message_id, turn, ended_at, failure, model, wait_key FROM orchestrator_entries;
DROP TABLE orchestrator_entries;
ALTER TABLE orchestrator_entries_reports RENAME TO orchestrator_entries;
CREATE INDEX orchestrator_entries_by_chat ON orchestrator_entries (chat_id, seq);
";
