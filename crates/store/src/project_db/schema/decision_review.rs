//! Persist the selected decision model/threshold and the precise failures of its reviews.
pub(super) const MIGRATION: &str = r"
ALTER TABLE decision_reviewer_connection ADD COLUMN model TEXT;
ALTER TABLE decision_reviewer_connection ADD COLUMN threshold REAL NOT NULL DEFAULT 0.8
  CHECK (threshold >= 0 AND threshold <= 1);
ALTER TABLE reviewer_usage ADD COLUMN decision_failure TEXT;
";
