//! Modules of a project's tasks (ORC-48, `orchester_core::module`): the modules, the module each task is in, and
//! every move of a task into or out of a module, which a module's burndown reads its scope changes from. Dates are
//! local days, `YYYY-MM-DD`.

pub(super) const MIGRATION: &str = r"
CREATE TABLE modules (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  normalized TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL,
  start_date TEXT CHECK (start_date IS NULL OR length(start_date) = 10),
  target_date TEXT CHECK (target_date IS NULL OR length(target_date) = 10),
  state TEXT NOT NULL CHECK (state IN ('open', 'closed')),
  created_at INTEGER NOT NULL,
  CHECK (start_date IS NULL OR target_date IS NULL OR target_date >= start_date)
) STRICT;

CREATE TABLE task_modules (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id) ON DELETE CASCADE,
  module_id TEXT NOT NULL REFERENCES modules (id)
) STRICT;
CREATE INDEX task_modules_by_module ON task_modules (module_id);

CREATE TABLE module_changes (
  sequence INTEGER PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  from_module TEXT REFERENCES modules (id),
  to_module TEXT REFERENCES modules (id),
  actor TEXT NOT NULL CHECK (actor IN ('person', 'orchester')),
  changed_at INTEGER NOT NULL,
  CHECK (from_module IS NOT to_module)
) STRICT;
CREATE INDEX module_changes_from ON module_changes (from_module, sequence);
CREATE INDEX module_changes_to ON module_changes (to_module, sequence);
CREATE TRIGGER module_changes_no_update BEFORE UPDATE ON module_changes
BEGIN SELECT RAISE(ABORT, 'module changes are append-only'); END;
CREATE TRIGGER module_changes_no_delete BEFORE DELETE ON module_changes
BEGIN SELECT RAISE(ABORT, 'module changes are append-only'); END;
";
