//! A run's execution inputs, kept when the run is created (team authority P3,
//! `docs/architecture/team-authority-plan.md` §7, `orchester_core::run::inputs`): the canonical content accepted for
//! it, as its canonical bytes and their hash; how this computer binds that content; and what a person here narrowed or
//! consented to. Three tables, so a local choice never reads as part of what was accepted. They hold references, never
//! a secret value, and never change: a team run accepted by an operation is that operation's only run.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_bundles (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  origin TEXT NOT NULL CHECK (origin IN ('local', 'team')),
  operation_id TEXT UNIQUE CHECK ((origin = 'team') = (operation_id IS NOT NULL)),
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
  body TEXT NOT NULL
) STRICT;
CREATE TRIGGER run_bundles_no_update BEFORE UPDATE ON run_bundles
BEGIN SELECT RAISE(ABORT, 'run inputs are immutable'); END;
CREATE TRIGGER run_bundles_no_delete BEFORE DELETE ON run_bundles
BEGIN SELECT RAISE(ABORT, 'run inputs are immutable'); END;

CREATE TABLE run_local_bindings (
  run_id TEXT PRIMARY KEY REFERENCES run_bundles (run_id),
  body TEXT NOT NULL
) STRICT;
CREATE TRIGGER run_local_bindings_no_update BEFORE UPDATE ON run_local_bindings
BEGIN SELECT RAISE(ABORT, 'run inputs are immutable'); END;
CREATE TRIGGER run_local_bindings_no_delete BEFORE DELETE ON run_local_bindings
BEGIN SELECT RAISE(ABORT, 'run inputs are immutable'); END;

CREATE TABLE run_local_consent (
  run_id TEXT PRIMARY KEY REFERENCES run_bundles (run_id),
  body TEXT NOT NULL
) STRICT;
CREATE TRIGGER run_local_consent_no_update BEFORE UPDATE ON run_local_consent
BEGIN SELECT RAISE(ABORT, 'run inputs are immutable'); END;
CREATE TRIGGER run_local_consent_no_delete BEFORE DELETE ON run_local_consent
BEGIN SELECT RAISE(ABORT, 'run inputs are immutable'); END;
";
