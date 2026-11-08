//! New queued tasks pin the selected published pipeline version.

pub(super) const MIGRATION: &str = r"
-- New queued tasks pin the selected published pipeline version. Existing queued rows have no
-- recoverable selection and remain NULL until the user explicitly resolves them.
ALTER TABLE tasks ADD COLUMN pipeline_version INTEGER CHECK (pipeline_version > 0);
";
