//! The plan limits that hold the Orchestrator's turns (`orchester_core::orchestrator::hold`,
//! `docs/plan/orchestrator-autonomy/OA-03-…`).

pub(super) const MIGRATION: &str = r"
-- An account the Orchestrator's CLI reported at its plan limit, since when, and when the CLI said the limit lifts
-- (NULL: it did not say). One row per account; gone once a turn on that account answers again.
CREATE TABLE orchestrator_plan_holds (
  account_id TEXT PRIMARY KEY,
  since INTEGER NOT NULL,
  resets_at INTEGER
) STRICT;
";
