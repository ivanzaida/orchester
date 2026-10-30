//! The disk guard for every project (ORC-422): the free space it keeps is a setting of its own, no longer part of the
//! warm workspaces settings, so it holds whether the project's runs borrow workspaces or own a worktree.

pub(super) const MIGRATION: &str = r"
-- Project settings › Runs › Storage: the free space, in whole GB, the disk guard keeps on the repository's disk.
-- Without a row it keeps 50 GB. A project that had warm workspaces on kept what it chose for them; the value saved
-- while they were off guarded nothing, so it is not taken.
CREATE TABLE disk_guard (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  keep_free_gb INTEGER NOT NULL CHECK (keep_free_gb > 0)
) STRICT;
INSERT INTO disk_guard (id, keep_free_gb) SELECT 1, keep_free_gb FROM workspace_settings WHERE enabled = 1;
ALTER TABLE workspace_settings DROP COLUMN keep_free_gb;
";
