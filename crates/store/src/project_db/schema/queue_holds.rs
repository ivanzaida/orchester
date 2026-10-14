//! Where a person holds new starts (`orchester_core::task_group::QueueHolds`): the whole project (`project`) or one
//! reviewed group (`group:<key>`), and since when.

pub(super) const MIGRATION: &str = r"
CREATE TABLE queue_holds (
  scope TEXT PRIMARY KEY CHECK (scope = 'project' OR scope LIKE 'group:%'),
  held_at INTEGER NOT NULL
) STRICT;
";
