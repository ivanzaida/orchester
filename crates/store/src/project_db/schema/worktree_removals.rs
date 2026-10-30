//! ORC-63: when a run's own worktree was removed, by *Remove worktree*, a merge or Run again. An
//! interrupted run whose worktree was removed stays interrupted in its record, but it can never
//! continue and no longer holds its task. Runs whose worktree went before this was kept have no row.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_worktree_removals (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  removed_at INTEGER NOT NULL
) STRICT;
";

/// ORC-418: a run whose worktree removal could not finish (a file held open, git stopping with the folder half
/// deleted) keeps the folder that stayed and why, one row per run, so its removal never reads as done while the folder
/// is on disk. The row goes once a removal of the run's worktrees leaves nothing behind.
pub(super) const NOT_REMOVED_MIGRATION: &str = r"
CREATE TABLE run_worktrees_not_removed (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  path TEXT NOT NULL,
  reason TEXT,
  at INTEGER NOT NULL
) STRICT;
";
