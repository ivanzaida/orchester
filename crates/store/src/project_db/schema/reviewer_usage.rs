//! What auto mode's reviewer spends (decided 2026-10-07): one record per review that asked a model.

pub(super) const MIGRATION: &str = r"
-- One review by auto mode's reviewer that asked a model, written by the runner when the review ends: the
-- request it judged, the reviewer's provider and model, when it ran, its verdict and the tokens the provider
-- reported. NULL tokens: what it spent could not be read (unknown, never zero). Known tokens count toward
-- the run's, the step's and the project's daily budgets and the Usage dashboard like an agent's turn.
CREATE TABLE reviewer_usage (
  id INTEGER PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_execution_id TEXT NOT NULL REFERENCES step_executions (id),
  invocation_id TEXT NOT NULL REFERENCES invocations (id),
  approval_id TEXT NOT NULL REFERENCES tool_approvals (id),
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  ended_at INTEGER NOT NULL,
  input_tokens INTEGER CHECK (input_tokens >= 0),
  cached_input_tokens INTEGER CHECK (cached_input_tokens >= 0),
  output_tokens INTEGER CHECK (output_tokens >= 0),
  verdict TEXT NOT NULL CHECK (verdict IN ('approved', 'asked', 'timed_out', 'unreadable', 'could_not_run', 'no_reviewer')),
  CHECK ((input_tokens IS NULL) = (cached_input_tokens IS NULL) AND (input_tokens IS NULL) = (output_tokens IS NULL))
) STRICT;
CREATE INDEX reviewer_usage_by_time ON reviewer_usage (ended_at);
CREATE INDEX reviewer_usage_by_run ON reviewer_usage (run_id);
";
