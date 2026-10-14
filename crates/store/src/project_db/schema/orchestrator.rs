//! The project's Orchestrator chat (`orchester_core::orchestrator`): the connection and model it runs on, its
//! conversations, what each holds in the order it was kept, and what its turns spent.

pub(super) const MIGRATION: &str = r"
-- One row once a person chose what the Orchestrator runs on. The connection is not a reference: a connection
-- removed later leaves the choice for the settings to show as gone.
CREATE TABLE orchestrator_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  connection_id TEXT NOT NULL,
  model TEXT NOT NULL CHECK (model <> ''),
  effort TEXT CHECK (effort IS NULL OR effort <> '')
) STRICT;

-- The agent CLI session the turns continue is set by the first turn that reports one, with its connection and
-- that connection's account.
CREATE TABLE orchestrator_chats (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  session TEXT CHECK (session IS NULL OR session <> ''),
  session_connection_id TEXT,
  session_account_id TEXT,
  CHECK ((session IS NULL) = (session_connection_id IS NULL)),
  CHECK ((session IS NULL) = (session_account_id IS NULL))
) STRICT;

-- A person's message (with the id the app gave it and the state of its turn), a reply of the agent (with its
-- model), an action it took through a tool (its JSON) or an event of the conversation (its JSON).
CREATE TABLE orchestrator_entries (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id TEXT NOT NULL REFERENCES orchestrator_chats (id),
  kind TEXT NOT NULL CHECK (kind IN ('person', 'reply', 'action', 'event')),
  at INTEGER NOT NULL,
  body TEXT NOT NULL CHECK (body <> ''),
  message_id TEXT UNIQUE,
  turn TEXT CHECK (turn IS NULL OR turn IN ('thinking', 'answered', 'stopped', 'failed')),
  failure TEXT,
  model TEXT,
  CHECK ((kind = 'person') = (message_id IS NOT NULL AND turn IS NOT NULL)),
  CHECK ((turn = 'failed') = (failure IS NOT NULL)),
  CHECK ((kind = 'reply') = (model IS NOT NULL))
) STRICT;

CREATE INDEX orchestrator_entries_by_chat ON orchestrator_entries (chat_id, seq);

-- What a turn's CLI reported it spent, one row per report: counted toward the project's daily budget.
CREATE TABLE orchestrator_usage (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id TEXT NOT NULL REFERENCES orchestrator_chats (id),
  message_id TEXT NOT NULL,
  at INTEGER NOT NULL,
  input_tokens INTEGER NOT NULL CHECK (input_tokens >= 0),
  cached_input_tokens INTEGER NOT NULL CHECK (cached_input_tokens >= 0),
  output_tokens INTEGER NOT NULL CHECK (output_tokens >= 0)
) STRICT;

CREATE INDEX orchestrator_usage_by_time ON orchestrator_usage (at);
CREATE INDEX orchestrator_usage_by_message ON orchestrator_usage (message_id);
";
