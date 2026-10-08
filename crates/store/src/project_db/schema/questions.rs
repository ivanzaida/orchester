//! Agents' questions with suggested answers (ORC-83): one row per request, which may ask several.

pub(super) const MIGRATION: &str = r"
-- A request of an agent that asks the person questions with suggested answers, written by the runner:
-- who asked in which run, step execution and invocation, the questions as JSON, and whether the agent's
-- turn waits for the answer (`in_turn`: Claude Code's ask_question, Codex's requestUserInput) or its step
-- ended with the question (`step_end`: a native agent). Answered once, by the person in the app or by
-- automation through Orchester's MCP server; a request whose agent ended before an answer reached it is
-- interrupted. `message_id` is the person's message a step-end answer resumed the agent with.
CREATE TABLE questions (
  id TEXT NOT NULL,
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  invocation_id TEXT NOT NULL REFERENCES invocations (id),
  agent TEXT NOT NULL,
  questions TEXT NOT NULL,
  turn TEXT NOT NULL CHECK (turn IN ('in_turn', 'step_end')),
  asked_at INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting', 'answered', 'interrupted')),
  answers TEXT CHECK ((state = 'answered') = (answers IS NOT NULL)),
  answered_by TEXT CHECK ((state = 'answered') = (answered_by IS NOT NULL) AND answered_by IN ('person', 'automation')),
  ended_at INTEGER CHECK ((state = 'waiting') = (ended_at IS NULL)),
  message_id TEXT,
  PRIMARY KEY (run_id, id)
) STRICT;
CREATE INDEX questions_waiting ON questions (state, asked_at);
";
