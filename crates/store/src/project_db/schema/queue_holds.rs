//! Where new starts are held (`orchester_core::task_group::QueueHolds`): the whole project (`project`) or one
//! reviewed group (`group:<key>`), since when, and who held them (`person` or `automation`).

pub(super) const MIGRATION: &str = r"
CREATE TABLE queue_holds (
  scope TEXT PRIMARY KEY CHECK (scope = 'project' OR scope LIKE 'group:%'),
  held_at INTEGER NOT NULL,
  set_by TEXT NOT NULL CHECK (set_by IN ('person', 'automation'))
) STRICT;
";
