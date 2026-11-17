//! A team module's acceptance as this computer chose it (`orchester_core::module::delivery::AcceptanceChoice`,
//! `docs/plan/orchestrator-autonomy/OA-32-…`): the team server assigns a team module's branch and keeps neither its
//! acceptance pipeline nor *Deliver when accepted*, so they are kept here, as a local module's are on its row. Module
//! and pipeline ids are the team server's; nothing points at a row of `modules` or `resources`.

pub(super) const MIGRATION: &str = r"
CREATE TABLE team_module_acceptance (
  module_id             TEXT PRIMARY KEY,
  acceptance_pipeline   TEXT,
  deliver_when_accepted INTEGER NOT NULL DEFAULT 0 CHECK (deliver_when_accepted IN (0, 1))
) STRICT;
";
