//! Tasks started together from the scheduling review (02.05/02.06): the group's scheduling, its
//! aggregate token cap and hold, and each member task in its reviewed order. A task joins one group
//! at most.

pub(super) const MIGRATION: &str = r"
CREATE TABLE task_groups (key TEXT PRIMARY KEY, scheduling TEXT NOT NULL, cap INTEGER NOT NULL CHECK(cap > 0), hold TEXT) STRICT;
CREATE TABLE task_group_members (
  group_key TEXT NOT NULL REFERENCES task_groups(key),
  task_id TEXT NOT NULL UNIQUE REFERENCES tasks(id),
  position INTEGER NOT NULL,
  PRIMARY KEY(group_key, position)
) STRICT;
";

/// A group whose every member's first run has ended is finished: the runner stops looking at it.
pub(super) const FINISHED_MIGRATION: &str = r"
ALTER TABLE task_groups ADD COLUMN finished INTEGER NOT NULL DEFAULT 0 CHECK (finished IN (0, 1));
";
