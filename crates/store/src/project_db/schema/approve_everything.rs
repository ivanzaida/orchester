//! *With auto mode on, approve everything in each run* (ORC-199, `orchester_core::run::ApproveEverything`): who
//! turned it on; a project without a row has it off.

pub(super) const MIGRATION: &str = r"
CREATE TABLE project_approve_everything (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  turned_on_by TEXT NOT NULL CHECK (turned_on_by IN ('person', 'automation'))
) STRICT;
";
