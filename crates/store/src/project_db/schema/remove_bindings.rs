//! The reference bindings are gone (`docs/design/team-configuration.md`, "What it replaces"): a team version names no
//! id of a computer, and what each computer fills in is kept by name (`team_secret_values`, `team_connection_picks`,
//! `mcp_server_programs`). The table goes, and with it the references of versions cached before the model, which named
//! connections, MCP servers and secrets by their ids on the publisher's computer: the references are rebuilt for the
//! team's kinds only, without the name and role those ids needed. The local body of such a version stays as it was.
//!
//! A team resource keeps the slug the server gave it, whether or not one of this computer's own has it, so slugs are
//! not unique across the two: `resources` is rebuilt without its `UNIQUE (kind, slug)`, and the explicit checks that
//! create a resource refuse a slug already taken, which a team resource arriving does not move.

pub(super) const MIGRATION: &str = r"
DROP TABLE reference_bindings;

CREATE TABLE shared_version_references_new (
  version_id TEXT NOT NULL REFERENCES shared_version_bodies (version_id),
  position INTEGER NOT NULL CHECK (position >= 0),
  kind TEXT NOT NULL CHECK (kind IN ('cli_kind', 'secret_name') OR substr(kind, 1, 5) = 'team_'),
  target_id TEXT NOT NULL,
  PRIMARY KEY (version_id, position)
) STRICT;
INSERT INTO shared_version_references_new (version_id, position, kind, target_id)
  SELECT version_id, position, kind, target_id FROM shared_version_references
  WHERE kind IN ('cli_kind', 'secret_name') OR substr(kind, 1, 5) = 'team_';
DROP TABLE shared_version_references;
ALTER TABLE shared_version_references_new RENAME TO shared_version_references;

CREATE TABLE resources_new (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL
    CHECK (kind IN ('agent_profile', 'instruction', 'pipeline', 'tool', 'skill',
                    'test_suite', 'service', 'stack', 'team_reviewer', 'mcp_server')),
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  sharing TEXT CHECK (sharing IN ('shared', 'not_shared')),
  server_revision INTEGER
    CHECK (server_revision > 0 AND (sharing IS 'shared') = (server_revision IS NOT NULL))
) STRICT;
INSERT INTO resources_new (rowid, id, kind, slug, name, created_at, sharing, server_revision)
  SELECT rowid, id, kind, slug, name, created_at, sharing, server_revision FROM resources;
DROP TABLE resources;
ALTER TABLE resources_new RENAME TO resources;
CREATE INDEX resources_kind_slug ON resources (kind, slug);
";
