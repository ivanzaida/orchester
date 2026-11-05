//! The team's reviewer uses the existing resource draft and immutable-version tables. The table is rebuilt for the
//! kind's CHECK and keeps every column it had, a shared project's `sharing` and `server_revision` among them.

pub(super) const MIGRATION: &str = r"
CREATE TABLE resources_new (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL
    CHECK (kind IN ('agent_profile', 'instruction', 'pipeline', 'tool', 'skill',
                    'test_suite', 'service', 'stack', 'team_reviewer')),
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  sharing TEXT CHECK (sharing IN ('shared', 'not_shared')),
  server_revision INTEGER
    CHECK (server_revision > 0 AND (sharing IS 'shared') = (server_revision IS NOT NULL)),
  UNIQUE (kind, slug)
) STRICT;
INSERT INTO resources_new (rowid, id, kind, slug, name, created_at, sharing, server_revision)
  SELECT rowid, id, kind, slug, name, created_at, sharing, server_revision FROM resources;
DROP TABLE resources;
ALTER TABLE resources_new RENAME TO resources;
";
