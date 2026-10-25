//! Runs completed without a merge (ORC-93): a run that passed every step and committed nothing, whose result is its
//! report or work done outside the repository, or one a person completed anyway; who completed it and when. Beside
//! them the texts a run keeps by name: a completed run's report (`orchester_core::run::REPORT_ARTIFACT`).

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_completions (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  completed_by TEXT NOT NULL CHECK (completed_by IN ('person', 'orchestrator')),
  completed_at INTEGER NOT NULL
) STRICT;

CREATE TABLE run_artifacts (
  run_id TEXT NOT NULL REFERENCES runs (id),
  name TEXT NOT NULL,
  step_key TEXT,
  text TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (run_id, name)
) STRICT;
";
