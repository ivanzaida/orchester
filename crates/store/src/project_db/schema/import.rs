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
