//! What the task's page tells of a team start apart from its stage: the runner's last refusal of a start that waits
//! for it, kept by the start's operation (a start's row cannot change once it is settled, so a settled start's mark
//! lives in its own table), the mark that the person has seen what a settled start came to, and the index the page's
//! read of a task's newest start goes by.

pub(super) const MIGRATION: &str = r"
ALTER TABLE team_starts ADD COLUMN runner_refusal TEXT;
CREATE TABLE team_start_seen (
  operation_id TEXT PRIMARY KEY REFERENCES team_starts (operation_id),
  seen_at INTEGER NOT NULL
) STRICT;
CREATE INDEX team_starts_task ON team_starts (task_id, created_at);
";
