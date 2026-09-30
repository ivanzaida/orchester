pub(super) const MIGRATION: &str = r"
CREATE TABLE task_organization (
  task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;
CREATE TABLE project_tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  normalized TEXT NOT NULL UNIQUE
) STRICT;
CREATE TABLE organization_changes (
  sequence INTEGER PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  actor TEXT NOT NULL,
  changed_at INTEGER NOT NULL,
  old_value TEXT NOT NULL,
  new_value TEXT NOT NULL,
  old_tag_names TEXT NOT NULL,
  new_tag_names TEXT NOT NULL
) STRICT;
CREATE INDEX organization_changes_by_task ON organization_changes(task_id, sequence);
";
