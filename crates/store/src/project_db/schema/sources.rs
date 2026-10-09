//! Sources (M12): folders and files outside the worktree an agent may read without asking. The project's and each
//! task's are listed here, in the order they were set; a pipeline's and its steps' are in its versions. A run
//! snapshots the union when it starts, in its run snapshot. Paths name this machine's folders: they stay in this
//! database and are never shared.

pub(super) const MIGRATION: &str = r"
CREATE TABLE project_sources (
  position INTEGER PRIMARY KEY CHECK (position >= 0),
  path TEXT NOT NULL UNIQUE CHECK (trim(path) <> '')
) STRICT;

CREATE TABLE task_sources (
  task_id TEXT NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL CHECK (position >= 0),
  path TEXT NOT NULL CHECK (trim(path) <> ''),
  PRIMARY KEY (task_id, position),
  UNIQUE (task_id, path)
) STRICT;
";
