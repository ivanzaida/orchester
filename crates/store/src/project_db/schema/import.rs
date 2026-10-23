//! Resources imported from another project (06.04–06.06).

pub(super) const MIGRATION: &str = r"
-- Resources imported from another project (06.04-06.06): where each copy came from, for display
-- only, as the copy is independent of its source. `source_version` is the published version copied,
-- NULL for a draft. `import_key` makes an import safe to retry: a key already written creates nothing.
CREATE TABLE resource_imports (
  resource_id TEXT PRIMARY KEY REFERENCES resources (id),
  import_key TEXT NOT NULL,
  source_project_id TEXT NOT NULL,
  source_project_name TEXT NOT NULL,
  source_resource_id TEXT NOT NULL,
  source_version INTEGER CHECK (source_version > 0),
  imported_at INTEGER NOT NULL,
  UNIQUE (import_key, source_resource_id)
) STRICT;
";

pub(super) const RECORD_MIGRATION: &str = r"
-- What an import's result shows (06.06 `z3vhzo`), for display only. `source_resource_name` is the source
-- resource's name, which a renamed copy does not have (NULL for copies made before it was kept: their own
-- name). `import_resolutions` holds, per import and in plan order, each source resource, connection, MCP
-- server or secret the import did not copy: `target_id` is this project's own used in its place, NULL when
-- it was left unresolved.
ALTER TABLE resource_imports ADD COLUMN source_resource_name TEXT;
CREATE TABLE import_resolutions (
  import_key TEXT NOT NULL,
  position INTEGER NOT NULL CHECK (position >= 0),
  source_kind TEXT NOT NULL CHECK (source_kind IN ('resource', 'connection', 'mcp_server', 'secret')),
  source_id TEXT NOT NULL,
  source_name TEXT NOT NULL,
  target_id TEXT,
  PRIMARY KEY (import_key, position)
) STRICT;
";
