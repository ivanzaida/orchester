//! Writing to the Orchestrator while it answers (ORC-92): a person's message may wait for the turn under way in its
//! conversation (`queued`, `queued_next` for *Stop and send*, `not_taken` for a message sent now that the turn did not
//! take), be handed to that turn (`steering`) or be taken by it (`joined`). A check cannot change in place, so
//! `orchestrator_entries` is made again with its rows, which keep their `seq`; nothing refers to it.

pub(super) const MIGRATION: &str = r"
CREATE TABLE orchestrator_entries_queue (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id TEXT NOT NULL REFERENCES orchestrator_chats (id),
  kind TEXT NOT NULL CHECK (kind IN ('person', 'reply', 'action', 'event', 'wait', 'report')),
  at INTEGER NOT NULL,
  body TEXT NOT NULL CHECK (body <> ''),
  message_id TEXT UNIQUE,
  turn TEXT CHECK (turn IS NULL OR turn IN ('thinking', 'answered', 'stopped', 'failed', 'queued', 'queued_next',
    'not_taken', 'steering', 'joined')),
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
  CHECK ((kind = 'reply') = (model IS NOT NULL)),
  -- Only a person's message waits for a turn or joins one.
  CHECK (kind = 'person' OR turn IS NULL OR turn IN ('thinking', 'answered', 'stopped', 'failed'))
) STRICT;

INSERT INTO orchestrator_entries_queue
  (seq, chat_id, kind, at, body, message_id, turn, ended_at, failure, model, wait_key)
  SELECT seq, chat_id, kind, at, body, message_id, turn, ended_at, failure, model, wait_key FROM orchestrator_entries;
DROP TABLE orchestrator_entries;
ALTER TABLE orchestrator_entries_queue RENAME TO orchestrator_entries;
CREATE INDEX orchestrator_entries_by_chat ON orchestrator_entries (chat_id, seq);
";
