//! Base branches of tasks and modules (`orchester_core::integration::base`, `Base branch · redesign`): the project's
//! base branch moves to its open modules and is gone.

pub(super) const MIGRATION: &str = r"
-- A task's base branch: its module's ('inherit'), the branch checked out when its run starts, or a branch. Kept on this
-- computer: a shared task's is never sent and leaves its server revision alone.
ALTER TABLE tasks ADD COLUMN base_choice TEXT NOT NULL DEFAULT 'inherit'
  CHECK (base_choice IN ('inherit', 'checked_out', 'branch'));
ALTER TABLE tasks ADD COLUMN base_branch TEXT CHECK ((base_choice = 'branch') = (base_branch IS NOT NULL));

-- A module's base branch; NULL for the branch checked out when a run starts. Kept on this computer, as a task's.
ALTER TABLE modules ADD COLUMN base_branch TEXT;

-- Where each run's base branch came from: the task's, its module's, the checked-out branch, or, for runs recorded
-- before tasks and modules had one, the project's.
ALTER TABLE runs ADD COLUMN base_source TEXT NOT NULL DEFAULT 'checked_out'
  CHECK (base_source IN ('task', 'module', 'checked_out', 'project'));
ALTER TABLE runs ADD COLUMN base_module TEXT CHECK ((base_source = 'module') = (base_module IS NOT NULL));
-- As far as a run's record tells, it took the project's base branch when it recorded the branch still set.
UPDATE runs SET base_source = 'project'
  WHERE base_branch IS NOT NULL AND base_branch = (SELECT branch FROM project_base_branch);

-- The project's base branch moved to its open modules: the branch and when, for Project settings › General.
CREATE TABLE base_branch_move (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  branch TEXT NOT NULL,
  moved_at INTEGER NOT NULL
) STRICT;
-- The modules that took it.
CREATE TABLE base_branch_move_modules (
  module_id TEXT PRIMARY KEY REFERENCES modules (id) ON UPDATE CASCADE ON DELETE CASCADE
) STRICT;
INSERT INTO base_branch_move (id, branch, moved_at)
  SELECT 1, branch, CAST((julianday('now') - 2440587.5) * 86400000.0 AS INTEGER) FROM project_base_branch;
INSERT INTO base_branch_move_modules (module_id)
  SELECT id FROM modules WHERE state = 'open' AND base_branch IS NULL AND EXISTS (SELECT 1 FROM project_base_branch);
UPDATE modules SET base_branch = (SELECT branch FROM project_base_branch)
  WHERE id IN (SELECT module_id FROM base_branch_move_modules);
DROP TABLE project_base_branch;
";
