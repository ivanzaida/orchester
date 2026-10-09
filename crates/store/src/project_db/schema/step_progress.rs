//! The line an agent reports on what its step is doing (`report_progress`, M12), one per step execution: a later
//! report replaces it. The run's Summary shows it.

pub(super) const MIGRATION: &str = r"
CREATE TABLE step_progress (
  step_execution_id TEXT PRIMARY KEY REFERENCES step_executions (id),
  text TEXT NOT NULL CHECK (trim(text) <> ''),
  reported_at INTEGER NOT NULL
) STRICT;
";
