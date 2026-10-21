//! Story points on tasks (ORC-49): a task's points with who set them, what the estimator spent, and the effort of a
//! merged task beside the points it had.

pub(super) const MIGRATION: &str = r"
-- A task's story points, kept beside the task outside its revision. `source` 'person' is never replaced by
-- automation. Automation's carry its reason and `basis`, the brief and criteria they were estimated from.
CREATE TABLE task_estimates (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id),
  points INTEGER NOT NULL CHECK (points IN (1, 2, 3, 5, 8, 13, 21)),
  source TEXT NOT NULL CHECK (source IN ('person', 'automation')),
  reason TEXT,
  basis TEXT,
  set_at INTEGER NOT NULL,
  CHECK (source = 'automation' OR (reason IS NULL AND basis IS NULL))
) STRICT;

-- One request to the story-point estimator that asked a model (auto mode's reviewer's connection and model), written
-- by the runner. NULL tokens: what it spent could not be read. Known tokens count toward the project's daily budget.
CREATE TABLE estimate_usage (
  id INTEGER PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  basis TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  ended_at INTEGER NOT NULL,
  input_tokens INTEGER CHECK (input_tokens >= 0),
  cached_input_tokens INTEGER CHECK (cached_input_tokens >= 0),
  output_tokens INTEGER CHECK (output_tokens >= 0),
  failure TEXT CHECK (failure IN ('timed_out', 'unreadable', 'could_not_run', 'no_reviewer')),
  CHECK ((input_tokens IS NULL) = (cached_input_tokens IS NULL) AND (input_tokens IS NULL) = (output_tokens IS NULL))
) STRICT;
CREATE INDEX estimate_usage_by_time ON estimate_usage (ended_at);
CREATE INDEX estimate_usage_by_task ON estimate_usage (task_id);

-- What a merged task took over all its runs, beside the points it had when it was merged (NULL: none). Written when a
-- run of the task merges; a later merge of another run replaces it.
CREATE TABLE task_effort (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id),
  merged_at INTEGER NOT NULL,
  points INTEGER CHECK (points IN (1, 2, 3, 5, 8, 13, 21)),
  source TEXT CHECK (source IN ('person', 'automation')),
  tokens INTEGER NOT NULL CHECK (tokens >= 0),
  active_ms INTEGER NOT NULL CHECK (active_ms >= 0),
  attempts INTEGER NOT NULL CHECK (attempts >= 0),
  runs INTEGER NOT NULL CHECK (runs >= 0),
  CHECK ((points IS NULL) = (source IS NULL))
) STRICT;
";
