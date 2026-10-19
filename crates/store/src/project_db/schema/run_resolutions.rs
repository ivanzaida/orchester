//! A merge of a run's base branch that waits in the run's worktree for a person to resolve its conflicts (*Resolve
//! in editor*): while it is recorded the run neither continues (a return of its work, a follow-up) nor is merged, so
//! no agent works in a worktree a person is merging in.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_resolutions (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  started_at INTEGER NOT NULL
) STRICT;
";
