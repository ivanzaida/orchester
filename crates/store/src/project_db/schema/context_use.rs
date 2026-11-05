//! How full each agent session's context window is (ORC-304): an agent CLI or native invocation's latest reading, and
//! the Orchestrator chat's for the session its turns continue.

pub(super) const MIGRATION: &str = r"
-- The latest reading of an invocation: the tokens its latest model response took in, the context window (NULL when
-- nobody reported one and no model limit is known) and whether that window is an estimate of the model's limit.
-- An invocation whose CLI reported no tokens has no row.
CREATE TABLE invocation_contexts (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  used_tokens INTEGER NOT NULL CHECK (used_tokens > 0),
  window_tokens INTEGER CHECK (window_tokens IS NULL OR window_tokens > 0),
  estimated INTEGER NOT NULL CHECK (estimated IN (0, 1) AND (estimated = 0 OR window_tokens IS NOT NULL)),
  at INTEGER NOT NULL
) STRICT;

-- The latest reading of the session a conversation's turns continue, as above; NULL until a turn reported one, and
-- again once the conversation starts another session.
ALTER TABLE orchestrator_chats ADD COLUMN context_used INTEGER CHECK (context_used IS NULL OR context_used > 0);
ALTER TABLE orchestrator_chats ADD COLUMN context_window INTEGER
  CHECK (context_window IS NULL OR (context_window > 0 AND context_used IS NOT NULL));
ALTER TABLE orchestrator_chats ADD COLUMN context_estimated INTEGER
  CHECK ((context_estimated IS NULL) = (context_used IS NULL) AND (context_estimated IS NOT 1 OR context_window IS NOT NULL));
";
