//! Publishing a shared project's configuration from this computer (`docs/architecture/server.md`, "Shared
//! configuration", rows 36 and 72). `sharing.server_admin` says whether the person signed in from here is the server's
//! admin, who changes the team's configuration as the project's owner does. A version published here waits in
//! `shared_publishes` until the server numbers it: its row in `resource_versions` is used at once with a number of
//! its own here, which the server's number replaces, and which moves up when a teammate's version takes it. It names
//! the version it was edited from by its row, whose number is the one sent; a conflict keeps the latest number the
//! server answered, a refusal the server's error code.

pub(super) const MIGRATION: &str = r"
ALTER TABLE sharing ADD COLUMN server_admin INTEGER NOT NULL DEFAULT 0 CHECK (server_admin IN (0, 1));

CREATE TABLE shared_publishes (
  version_id TEXT PRIMARY KEY REFERENCES resource_versions (id),
  resource_id TEXT NOT NULL REFERENCES resources (id),
  idempotency_key TEXT NOT NULL UNIQUE,
  base_version_id TEXT REFERENCES resource_versions (id),
  conflict_latest INTEGER CHECK (conflict_latest > 0),
  refused TEXT,
  CHECK (conflict_latest IS NULL OR refused IS NULL)
) STRICT;

DROP TRIGGER resource_versions_no_update;
CREATE TRIGGER resource_versions_no_update BEFORE UPDATE ON resource_versions
WHEN NEW.id IS NOT OLD.id OR NEW.resource_id IS NOT OLD.resource_id
  OR (NEW.number IS NOT OLD.number AND OLD.id NOT IN (SELECT version_id FROM shared_publishes))
  OR NEW.body IS NOT OLD.body OR NEW.published_at IS NOT OLD.published_at OR OLD.publisher_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
";

/// *Open my draft on vN* takes back a version published here that the server never numbered, which no run or task here
/// uses: only such a version may be deleted. Every other version stays immutable.
pub(super) const REOPEN_MIGRATION: &str = r"
DROP TRIGGER resource_versions_no_delete;
CREATE TRIGGER resource_versions_no_delete BEFORE DELETE ON resource_versions
WHEN OLD.id NOT IN (SELECT version_id FROM shared_publishes)
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
";
