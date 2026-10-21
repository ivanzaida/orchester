//! ORC-75: why the runner could not record or carry on a run it interrupted (`orchester_core::run::RunnerFailure`),
//! kept with that interruption, the run's event, in the same transaction. Interruptions recorded before this have
//! no row: their kind is unknown.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_runner_failures (
  run_id TEXT NOT NULL,
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  kind TEXT NOT NULL CHECK (kind IN ('storage_contention', 'storage_failure', 'other')),
  PRIMARY KEY (run_id, sequence),
  FOREIGN KEY (run_id, sequence) REFERENCES run_events (run_id, sequence)
) STRICT;
";
