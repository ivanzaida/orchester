//! A run's own model and effort (ORC-322, `orchester_core::run::ModelChoice`): the model and effort a step of a run
//! runs its next turns on in place of its profile's, written as a person changes them from the run's Conversation and
//! read by the runner at each turn's start; and the changes and their cancelations, append-only, for the
//! Conversation's event lines.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_step_models (
  run_id TEXT NOT NULL REFERENCES runs (id),
  step_key TEXT NOT NULL,
  model TEXT NOT NULL,
  effort TEXT,
  PRIMARY KEY (run_id, step_key)
) STRICT;
CREATE TABLE run_model_events (
  id INTEGER PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  at INTEGER NOT NULL,
  by TEXT NOT NULL CHECK (by IN ('person', 'automation')),
  step_key TEXT NOT NULL,
  event TEXT NOT NULL
) STRICT;
CREATE INDEX run_model_events_run ON run_model_events (run_id, id);
CREATE TRIGGER run_model_events_no_update BEFORE UPDATE ON run_model_events
BEGIN SELECT RAISE(ABORT, 'run model events are append-only'); END;
CREATE TRIGGER run_model_events_no_delete BEFORE DELETE ON run_model_events
BEGIN SELECT RAISE(ABORT, 'run model events are append-only'); END;
";
