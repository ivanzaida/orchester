//! The outbox of the team's configuration goes (`docs/design/team-configuration-live.md`, rule 6): an admin's change of
//! a team resource is sent to the team server at once, and nothing of it waits on this computer. A version published
//! here that the server never numbered is dropped, except one a run or a task here uses, which stays as the row it
//! pinned (the server's version of its number takes its place when the stream keeps it). No version waits for a
//! person's *Use v<N> here* any more, and no draft of a team resource is kept here. The overrides this computer keeps
//! (connection picks, program paths, MCP switches, secret values, the fill-ins) are not touched. Running it again
//! changes nothing: the tables it drops are made empty first when they are gone already.

pub(super) const MIGRATION: &str = r"
CREATE TABLE IF NOT EXISTS shared_publishes (version_id TEXT, resource_id TEXT, created INTEGER);
CREATE TABLE IF NOT EXISTS waiting_versions (version_id TEXT);
CREATE TEMP TABLE unsent_versions AS SELECT version_id AS id FROM shared_publishes;
DROP TABLE waiting_versions;
DROP TABLE shared_publishes;

DROP TRIGGER IF EXISTS resource_versions_no_delete;
DELETE FROM resource_versions
  WHERE id IN (SELECT id FROM temp.unsent_versions)
  AND NOT EXISTS (SELECT 1 FROM runs WHERE runs.pipeline_version_id = resource_versions.id
    OR instr(runs.snapshot, resource_versions.id) > 0)
  AND NOT EXISTS (SELECT 1 FROM tasks WHERE tasks.pipeline_id = resource_versions.resource_id
    AND tasks.pipeline_version = resource_versions.number);
CREATE TRIGGER resource_versions_no_delete BEFORE DELETE ON resource_versions
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
DROP TABLE temp.unsent_versions;

DROP TRIGGER IF EXISTS resource_versions_no_update;
CREATE TRIGGER resource_versions_no_update BEFORE UPDATE ON resource_versions
WHEN NEW.id IS NOT OLD.id OR NEW.resource_id IS NOT OLD.resource_id OR NEW.number IS NOT OLD.number
  OR NEW.body IS NOT OLD.body OR NEW.published_at IS NOT OLD.published_at OR OLD.publisher_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;

DELETE FROM resource_drafts
  WHERE EXISTS (SELECT 1 FROM sharing)
  AND resource_id IN (SELECT id FROM resources WHERE sharing IS 'shared');
";
