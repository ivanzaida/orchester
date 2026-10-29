//! A shared project's configuration (`docs/architecture/server.md`, "Desktop data", S5): which resources are the
//! server's, each cached version twice — the server's body unchanged and the local body with this computer's bindings
//! in place of the references — and, per reference, the local connection, MCP server or secret it means here.

/// `resources.sharing` is empty for a local project's resource, `shared` for the server's (with its revision there)
/// and `not_shared` for one kept local at a join. A version's publisher is the server project's member who published
/// it; it may be set once on a version published here before the project was shared, nothing else of a version
/// changes. `resource_versions.body` stays the local body as first cached; `shared_version_bodies.local_body` is the
/// one the bindings rewrite, and reads take it.
pub(super) const MIGRATION: &str = r"
ALTER TABLE resources ADD COLUMN sharing TEXT CHECK (sharing IN ('shared', 'not_shared'));
ALTER TABLE resources ADD COLUMN server_revision INTEGER
  CHECK (server_revision > 0 AND (sharing IS 'shared') = (server_revision IS NOT NULL));
ALTER TABLE resource_versions ADD COLUMN publisher_id TEXT;
ALTER TABLE resource_versions ADD COLUMN publisher_name TEXT
  CHECK ((publisher_id IS NULL) = (publisher_name IS NULL));

DROP TRIGGER resource_versions_no_update;
CREATE TRIGGER resource_versions_no_update BEFORE UPDATE ON resource_versions
WHEN NEW.id IS NOT OLD.id OR NEW.resource_id IS NOT OLD.resource_id OR NEW.number IS NOT OLD.number
  OR NEW.body IS NOT OLD.body OR NEW.published_at IS NOT OLD.published_at OR OLD.publisher_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;

CREATE TABLE shared_version_bodies (
  version_id TEXT PRIMARY KEY REFERENCES resource_versions (id),
  server_body TEXT NOT NULL,
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
  local_body TEXT NOT NULL
) STRICT;

-- What a cached version names that is not a shared resource of the project, or a shared tool, in its order there.
CREATE TABLE shared_version_references (
  version_id TEXT NOT NULL REFERENCES shared_version_bodies (version_id),
  position INTEGER NOT NULL CHECK (position >= 0),
  kind TEXT NOT NULL CHECK (kind IN ('connection', 'mcp_server', 'secret', 'tool')),
  target_id TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT,
  PRIMARY KEY (version_id, position)
) STRICT;

-- Per reference, by its kind and its id on the publisher's computer: the local one it means here (none while unbound)
-- and how it was bound, by name, by role or by a person.
CREATE TABLE reference_bindings (
  kind TEXT NOT NULL CHECK (kind IN ('connection', 'mcp_server', 'secret')),
  reference_id TEXT NOT NULL,
  name TEXT NOT NULL,
  role TEXT,
  local_id TEXT,
  bound_by TEXT CHECK (bound_by IN ('name', 'role', 'person')),
  changed_at INTEGER NOT NULL,
  PRIMARY KEY (kind, reference_id),
  CHECK ((local_id IS NULL) = (bound_by IS NULL))
) STRICT;
";
