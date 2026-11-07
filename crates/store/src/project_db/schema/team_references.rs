//! A cached version names a CLI kind, a team resource by its server id or a secret by name
//! (`docs/design/team-configuration.md`), next to the ids on the publisher's computer that versions cached before kept.
//! The table is rebuilt for the kind's CHECK. A team resource's kind is `team_` and its resource kind, so a shared tool
//! the table held as `tool` becomes `team_tool`.

pub(super) const MIGRATION: &str = r"
CREATE TABLE shared_version_references_new (
  version_id TEXT NOT NULL REFERENCES shared_version_bodies (version_id),
  position INTEGER NOT NULL CHECK (position >= 0),
  kind TEXT NOT NULL
    CHECK (kind IN ('connection', 'mcp_server', 'secret', 'cli_kind', 'secret_name') OR substr(kind, 1, 5) = 'team_'),
  target_id TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT,
  PRIMARY KEY (version_id, position)
) STRICT;
INSERT INTO shared_version_references_new (version_id, position, kind, target_id, name, role)
  SELECT version_id, position, CASE kind WHEN 'tool' THEN 'team_tool' ELSE kind END, target_id, name, role
  FROM shared_version_references;
DROP TABLE shared_version_references;
ALTER TABLE shared_version_references_new RENAME TO shared_version_references;
";
