//! Skills, the repository agent files setting and imports from the repository (ORC-15).

pub(super) const MIGRATION: &str = r"
-- Skills are a resource kind. SQLite cannot change a CHECK constraint, so the table is rebuilt,
-- its rows keeping their `rowid` (imports list the resources they created in that order).
CREATE TABLE resources_new (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('agent_profile', 'instruction', 'pipeline', 'tool', 'skill')),
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (kind, slug)
) STRICT;
INSERT INTO resources_new (rowid, id, kind, slug, name, created_at)
  SELECT rowid, id, kind, slug, name, created_at FROM resources;
DROP TABLE resources;
ALTER TABLE resources_new RENAME TO resources;

-- Project settings › General › Repository agent files; without a row the project has it on.
CREATE TABLE project_agent_files (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  setting TEXT NOT NULL CHECK (setting IN ('on', 'off'))
) STRICT;

-- Resources made from the repository's own agent files (Import from repository): the file or folder
-- each came from, for display only, as the draft does not follow it. `import_key` makes an import
-- safe to retry: a key already written creates nothing.
CREATE TABLE repository_imports (
  resource_id TEXT PRIMARY KEY REFERENCES resources (id),
  import_key TEXT NOT NULL,
  source TEXT NOT NULL,
  imported_at INTEGER NOT NULL
) STRICT;
CREATE INDEX repository_imports_by_key ON repository_imports (import_key);
";
