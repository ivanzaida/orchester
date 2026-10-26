//! Removed resources (ORC-107): a removed agent profile, instruction, skill or tool keeps its row, slug and versions,
//! so the runs that pinned it still read them, but no list shows it and no new run pins it.

pub(super) const MIGRATION: &str = r"
CREATE TABLE removed_resources (
  resource_id TEXT PRIMARY KEY REFERENCES resources (id),
  removed_at INTEGER NOT NULL
) STRICT;
";
