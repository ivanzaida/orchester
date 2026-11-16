//! A module's own branch, its deliveries and its acceptances (`orchester_core::module::delivery`,
//! `docs/plan/orchestrator-autonomy/OA-17-…`, `OA-20-…`).

pub(super) const MIGRATION: &str = r"
-- A module's own branch (`module/<slug>`), the pipeline its acceptance runs on, and whether the conversation that
-- leads it may deliver it once accepted. Kept on this computer, as a module's base branch is; NULL: the module's
-- tasks merge into its base branch, as every module's did.
ALTER TABLE modules ADD COLUMN own_branch TEXT;
ALTER TABLE modules ADD COLUMN acceptance_pipeline TEXT;
ALTER TABLE modules ADD COLUMN deliver_when_accepted INTEGER NOT NULL DEFAULT 0
  CHECK (deliver_when_accepted IN (0, 1));

-- Each merge of a module's own branch into its base branch that this computer made.
CREATE TABLE module_deliveries (
  id INTEGER PRIMARY KEY,
  module_id TEXT NOT NULL,
  head TEXT NOT NULL,
  into_branch TEXT NOT NULL,
  merge_commit TEXT NOT NULL,
  at INTEGER NOT NULL
) STRICT;
CREATE INDEX module_deliveries_by_module ON module_deliveries (module_id, at);

-- The tasks Orchester made to accept a module: one run each, over the module's whole diff.
CREATE TABLE module_acceptances (
  task_id TEXT PRIMARY KEY,
  module_id TEXT NOT NULL,
  at INTEGER NOT NULL
) STRICT;
CREATE INDEX module_acceptances_by_module ON module_acceptances (module_id, at);
";
