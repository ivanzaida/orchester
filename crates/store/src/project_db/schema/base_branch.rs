//! The project's base branch (ORC-81, Project settings › General › Base branch).

pub(super) const MIGRATION: &str = r"
-- The branch new runs start from and deliver into, written when a person sets it; without a row they take the branch
-- the checkout has checked out when they start. Kept on this computer only: a shared project does not sync it.
CREATE TABLE project_base_branch (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  branch TEXT NOT NULL
) STRICT;
";
