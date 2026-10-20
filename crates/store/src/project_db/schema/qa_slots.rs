//! Project settings › Limits › QA apps at once (ORC-64): how many QA apps the project's runs may have open at once,
//! saved with the project's other limits.

pub(super) const MIGRATION: &str = r"
-- Without a row the project uses the defaults; a row saved before this keeps the default of 2.
ALTER TABLE project_limits ADD COLUMN qa_slots INTEGER NOT NULL DEFAULT 2 CHECK (qa_slots BETWEEN 1 AND 16);
";
