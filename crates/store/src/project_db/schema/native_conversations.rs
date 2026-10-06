//! Native agents (`docs/architecture/native-agent.md` §6.2): the conversation of each step execution
//! and its items, append-only like the transcript, and what each native invocation reached its model
//! through (04.H10).

pub(super) const MIGRATION: &str = r"
CREATE TABLE native_conversations (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  connection_id TEXT NOT NULL,
  wire TEXT NOT NULL CHECK (wire IN ('anthropic_messages', 'openai_responses')),
  model TEXT NOT NULL,
  -- The system prompt, frozen for the conversation.
  system TEXT NOT NULL,
  -- The repository access its tools were chosen for.
  access TEXT NOT NULL CHECK (access IN ('read_only', 'write')),
  created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX native_conversations_by_execution ON native_conversations (step_execution_id);

CREATE TABLE native_items (
  conversation_id TEXT NOT NULL REFERENCES native_conversations (id),
  seq INTEGER NOT NULL CHECK (seq > 0),
  invocation_id TEXT NOT NULL REFERENCES invocations (id),
  -- Increments at each compaction item.
  segment INTEGER NOT NULL CHECK (segment >= 0),
  kind TEXT NOT NULL
    CHECK (kind IN ('user', 'assistant', 'reasoning', 'tool_call', 'tool_result', 'compaction')),
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (conversation_id, seq)
) STRICT;

CREATE TRIGGER native_conversations_immutable BEFORE UPDATE ON native_conversations
BEGIN SELECT RAISE(ABORT, 'a native conversation is never changed'); END;
CREATE TRIGGER native_conversations_kept BEFORE DELETE ON native_conversations
BEGIN SELECT RAISE(ABORT, 'a native conversation is never deleted'); END;
CREATE TRIGGER native_items_immutable BEFORE UPDATE ON native_items
BEGIN SELECT RAISE(ABORT, 'native conversation items are append-only'); END;
CREATE TRIGGER native_items_kept BEFORE DELETE ON native_items
BEGIN SELECT RAISE(ABORT, 'native conversation items are append-only'); END;
CREATE TRIGGER native_items_gapless BEFORE INSERT ON native_items
WHEN NEW.seq <> 1 + COALESCE((SELECT MAX(seq) FROM native_items WHERE conversation_id = NEW.conversation_id), 0)
BEGIN SELECT RAISE(ABORT, 'native conversation items are numbered without gaps'); END;

-- The endpoint, vendor and effort a native invocation used, written once before it starts.
CREATE TABLE invocation_engine (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  body TEXT NOT NULL
) STRICT;
CREATE TRIGGER invocation_engine_immutable BEFORE UPDATE ON invocation_engine
BEGIN SELECT RAISE(ABORT, 'an invocation keeps the engine it started with'); END;
";
