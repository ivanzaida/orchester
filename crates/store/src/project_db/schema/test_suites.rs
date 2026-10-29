//! Test suites (ORC-244, `docs/architecture/agent-testing.md`): a resource kind, with drafts and
//! published versions as every other.

pub(super) const MIGRATION: &str = r"
-- SQLite cannot change a CHECK constraint, so the table is rebuilt, its rows keeping their `rowid`
-- (imports list the resources they created in that order).
CREATE TABLE resources_new (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL
    CHECK (kind IN ('agent_profile', 'instruction', 'pipeline', 'tool', 'skill', 'test_suite')),
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (kind, slug)
) STRICT;
INSERT INTO resources_new (rowid, id, kind, slug, name, created_at)
  SELECT rowid, id, kind, slug, name, created_at FROM resources;
DROP TABLE resources;
ALTER TABLE resources_new RENAME TO resources;
";
