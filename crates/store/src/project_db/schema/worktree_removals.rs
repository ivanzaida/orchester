//! ORC-63: when a run's own worktree was removed, by *Remove worktree*, a merge or Run again. An
//! interrupted run whose worktree was removed stays interrupted in its record, but it can never
//! continue and no longer holds its task. Runs whose worktree went before this was kept have no row.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_worktree_removals (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  removed_at INTEGER NOT NULL
) STRICT;
";
