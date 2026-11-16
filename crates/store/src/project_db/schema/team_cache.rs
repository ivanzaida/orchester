//! The copy of the team's configuration goes (`docs/design/team-configuration-live.md`): the team server is the only
//! source of a team project's configuration, read into memory, so the project database of a shared project keeps none
//! of the team's resources, versions, bodies or references. A version a run or a task here still pins stays as the row
//! it pinned, with its resource, its bodies and its references; every other version of the team's resources goes, and
//! each team resource left with no version goes with what names it by its id (drafts, archive marks, imports, the
//! default pipeline), and the tasks that named it as their own pipeline name none. What this computer fills in for the
//! team's configuration (the fill-ins, connection picks, MCP switches, secret values, program paths) is keyed by the
//! team resource's id and names no row here, so it stays. A local project has no team resources, and is not touched.
//! Running it again changes nothing.

pub(super) const MIGRATION: &str = r"
CREATE TEMP TABLE team_cache AS
  SELECT id FROM resources WHERE sharing IS 'shared' AND EXISTS (SELECT 1 FROM sharing);
CREATE TEMP TABLE pinned_versions AS
  SELECT id FROM resource_versions
  WHERE resource_id IN (SELECT id FROM temp.team_cache)
  AND (EXISTS (SELECT 1 FROM runs WHERE runs.pipeline_version_id = resource_versions.id
      OR instr(runs.snapshot, resource_versions.id) > 0)
    OR EXISTS (SELECT 1 FROM tasks WHERE tasks.pipeline_id = resource_versions.resource_id
      AND tasks.pipeline_version = resource_versions.number));
CREATE TEMP TABLE dropped_versions AS
  SELECT id FROM resource_versions
  WHERE resource_id IN (SELECT id FROM temp.team_cache) AND id NOT IN (SELECT id FROM temp.pinned_versions);

DELETE FROM shared_version_references WHERE version_id IN (SELECT id FROM temp.dropped_versions);
DELETE FROM shared_version_bodies WHERE version_id IN (SELECT id FROM temp.dropped_versions);
DROP TRIGGER IF EXISTS resource_versions_no_delete;
DELETE FROM resource_versions WHERE id IN (SELECT id FROM temp.dropped_versions);
CREATE TRIGGER resource_versions_no_delete BEFORE DELETE ON resource_versions
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;

CREATE TEMP TABLE dropped_resources AS
  SELECT id FROM temp.team_cache
  WHERE NOT EXISTS (SELECT 1 FROM resource_versions WHERE resource_versions.resource_id = team_cache.id);
UPDATE tasks SET pipeline_id = NULL WHERE pipeline_id IN (SELECT id FROM temp.dropped_resources);
DELETE FROM project_default_pipeline WHERE pipeline_id IN (SELECT id FROM temp.dropped_resources);
DELETE FROM resource_drafts WHERE resource_id IN (SELECT id FROM temp.dropped_resources);
DELETE FROM removed_resources WHERE resource_id IN (SELECT id FROM temp.dropped_resources);
DELETE FROM repository_imports WHERE resource_id IN (SELECT id FROM temp.dropped_resources);
DELETE FROM automation_drafts WHERE resource_id IN (SELECT id FROM temp.dropped_resources);
DELETE FROM resource_imports WHERE resource_id IN (SELECT id FROM temp.dropped_resources);
DELETE FROM resources WHERE id IN (SELECT id FROM temp.dropped_resources);

DROP TABLE temp.dropped_resources;
DROP TABLE temp.dropped_versions;
DROP TABLE temp.pinned_versions;
DROP TABLE temp.team_cache;
";
