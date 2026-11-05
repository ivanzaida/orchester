pub(super) const MIGRATION: &str = r"
ALTER TABLE decision_reviewer_connection ADD COLUMN settings TEXT;
";
