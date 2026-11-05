//! Side questions about a run (ORC-337): kept with their task, never in the run's conversation. A question is asked,
//! then answered or failed once; an answer is sent on to the run's agent at most once.

pub(super) const MIGRATION: &str = r"
CREATE TABLE side_questions (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  run_id TEXT NOT NULL REFERENCES runs (id),
  question TEXT NOT NULL,
  asked_at INTEGER NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('answering', 'answered', 'failed')),
  -- The answer as JSON (its text and sources) and the model that gave it, once answered.
  answer TEXT,
  model TEXT,
  -- Why it got no answer, as JSON, once failed.
  failure TEXT,
  ended_at INTEGER,
  -- When the person sent the answer on to the run's agent.
  sent_at INTEGER,
  CHECK ((state = 'answered') = (answer IS NOT NULL AND model IS NOT NULL)),
  CHECK ((state = 'failed') = (failure IS NOT NULL)),
  CHECK ((state = 'answering') = (ended_at IS NULL)),
  CHECK (sent_at IS NULL OR state = 'answered')
) STRICT;
CREATE INDEX side_questions_by_task ON side_questions (task_id, asked_at);
";
