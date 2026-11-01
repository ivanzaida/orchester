//! The team's reviewer uses the existing resource draft and immutable-version tables.

pub(super) const MIGRATION: &str = r"
CREATE TABLE resources_new (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL
    CHECK (kind IN ('agent_profile', 'instruction', 'pipeline', 'tool', 'skill',
                    'test_suite', 'service', 'stack', 'team_reviewer')),
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
