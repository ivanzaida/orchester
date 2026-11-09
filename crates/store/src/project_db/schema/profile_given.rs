//! A local agent profile owns what its agents are given (`docs/design/team-configuration.md`, "Decided 2026-11-09"): it
//! chooses the project's instructions, skills and tools, and nothing attaches itself any more. Every local profile,
//! each published version and its draft, gets what reached its agents before, read from the latest published version
//! of every instruction, skill, tool and pipeline that is not archived:
//!
//! - an instruction whose scope admits the profile (the project; the profile's role; a pipeline one of whose steps it
//!   runs; that step of that pipeline) and whose targets select it (all agents, its role, or it by id);
//! - a skill whose scope admits it, by the same rule;
//! - a tool whose access rules let a pipeline step it runs call it.
//!
//! Which steps a profile runs is read from the latest published version of each pipeline and from the snapshot of
//! every run that has not succeeded or been cancelled, so a run in flight that pinned an older pipeline keeps what its
//! agents got. The role is the version's own. A list is ordered by the resources' names and left out while empty, as
//! the body's shape has it. A body of schema 3 to 5 has the shape of schema 6 and takes its number; an older one keeps
//! its own, whose upgrade keeps the lists.
//!
//! The published versions are changed in place, not followed by a new one: a local run reads the profile version it
//! pinned when its agent starts (`GivenBy::Profile`), so a new version would leave every run already pinned to an
//! older one with nothing. The immutability trigger is dropped for the update and created again as it was. A team's
//! cached versions and versions waiting for the team server are left as they are: a team run is given by its bundle.
//! Running it again computes the same lists from the same rows and writes them over themselves.

pub(super) const MIGRATION: &str = r"
CREATE TEMP TABLE given_latest AS
  SELECT resources.id AS resource_id, resources.kind AS kind, resources.name AS name, versions.body AS body
  FROM resources JOIN resource_versions AS versions ON versions.resource_id = resources.id
  WHERE resources.kind IN ('instruction', 'skill', 'tool', 'pipeline')
    AND resources.sharing IS NOT 'shared'
    AND resources.id NOT IN (SELECT resource_id FROM removed_resources)
    AND versions.number = (SELECT MAX(number) FROM resource_versions WHERE resource_id = resources.id);

CREATE TEMP TABLE given_steps AS
  SELECT latest.resource_id AS pipeline, json_extract(step.value, '$.key') AS step,
    json_extract(step.value, '$.kind.agent.profile') AS profile
  FROM given_latest AS latest, json_each(latest.body, '$.body.steps') AS step
  WHERE latest.kind = 'pipeline' AND json_extract(step.value, '$.kind.agent.profile') IS NOT NULL
  UNION
  SELECT json_extract(runs.snapshot, '$.body.pipeline.resource'), json_extract(step.value, '$.key'),
    json_extract(step.value, '$.executor.profile.resource')
  FROM runs, json_each(runs.snapshot, '$.body.steps') AS step
  WHERE runs.status NOT IN ('succeeded', 'cancelled') AND json_extract(step.value, '$.executor.type') = 'agent';

CREATE TEMP TABLE given_profiles AS
  SELECT 'version' AS place, versions.id AS row_id, versions.resource_id AS profile,
    json_extract(versions.body, '$.body.role') AS role, versions.body AS body
  FROM resource_versions AS versions JOIN resources ON resources.id = versions.resource_id
  WHERE resources.kind = 'agent_profile' AND resources.sharing IS NOT 'shared' AND versions.publisher_id IS NULL
    AND versions.id NOT IN (SELECT version_id FROM shared_publishes)
  UNION ALL
  SELECT 'draft', drafts.resource_id, drafts.resource_id, json_extract(drafts.body, '$.body.role'), drafts.body
  FROM resource_drafts AS drafts JOIN resources ON resources.id = drafts.resource_id
  WHERE resources.kind = 'agent_profile' AND resources.sharing IS NOT 'shared';

-- Whether the scope of the body `latest` admits the profile `given`.
CREATE TEMP VIEW given_admitted AS
  SELECT given.place AS place, given.row_id AS row_id, latest.resource_id AS resource_id
  FROM given_profiles AS given, given_latest AS latest
  WHERE latest.kind IN ('instruction', 'skill')
    AND (json_type(latest.body, '$.body.scope') = 'text'
      OR json_extract(latest.body, '$.body.scope.agent_role.role') = given.role
      OR EXISTS (SELECT 1 FROM given_steps AS steps WHERE steps.profile = given.profile
        AND steps.pipeline = json_extract(latest.body, '$.body.scope.pipeline.pipeline'))
      OR EXISTS (SELECT 1 FROM given_steps AS steps WHERE steps.profile = given.profile
        AND steps.pipeline = json_extract(latest.body, '$.body.scope.step.pipeline')
        AND steps.step = json_extract(latest.body, '$.body.scope.step.step')))
    AND (latest.kind = 'skill'
      OR json_type(latest.body, '$.body.targets') = 'text'
      OR EXISTS (SELECT 1 FROM json_each(latest.body, '$.body.targets.roles') WHERE value = given.role)
      OR EXISTS (SELECT 1 FROM json_each(latest.body, '$.body.targets.profiles') WHERE value = given.profile));

CREATE TEMP VIEW given_callable AS
  SELECT given.place AS place, given.row_id AS row_id, latest.resource_id AS resource_id
  FROM given_profiles AS given, given_latest AS latest
  WHERE latest.kind = 'tool'
    AND EXISTS (SELECT 1 FROM json_each(latest.body, '$.body.access.callers') AS caller
      JOIN given_steps AS steps ON steps.pipeline = caller.key AND steps.profile = given.profile
      WHERE caller.type = 'text'
        OR EXISTS (SELECT 1 FROM json_each(caller.value, '$.only') WHERE value = steps.step));

CREATE TEMP TABLE given_bodies AS
  SELECT given.place AS place, given.row_id AS row_id,
    CASE WHEN json_extract(given.body, '$.schema') BETWEEN 3 AND 5 THEN json_set(given.body, '$.schema', 6)
      ELSE given.body END AS body,
    (SELECT json_group_array(latest.resource_id ORDER BY lower(latest.name), latest.resource_id)
      FROM given_admitted AS admitted JOIN given_latest AS latest ON latest.resource_id = admitted.resource_id
      WHERE admitted.place = given.place AND admitted.row_id = given.row_id AND latest.kind = 'instruction')
      AS instructions,
    (SELECT json_group_array(latest.resource_id ORDER BY lower(latest.name), latest.resource_id)
      FROM given_admitted AS admitted JOIN given_latest AS latest ON latest.resource_id = admitted.resource_id
      WHERE admitted.place = given.place AND admitted.row_id = given.row_id AND latest.kind = 'skill')
      AS skills,
    (SELECT json_group_array(latest.resource_id ORDER BY lower(latest.name), latest.resource_id)
      FROM given_callable AS callable JOIN given_latest AS latest ON latest.resource_id = callable.resource_id
      WHERE callable.place = given.place AND callable.row_id = given.row_id)
      AS tools
  FROM given_profiles AS given;
UPDATE given_bodies SET body = json_set(body, '$.body.instruction_resources', json(instructions))
  WHERE instructions <> '[]';
UPDATE given_bodies SET body = json_set(body, '$.body.skills', json(skills)) WHERE skills <> '[]';
UPDATE given_bodies SET body = json_set(body, '$.body.tools', json(tools)) WHERE tools <> '[]';

DROP TRIGGER resource_versions_no_update;
UPDATE resource_versions
  SET body = (SELECT body FROM given_bodies WHERE place = 'version' AND row_id = resource_versions.id)
  WHERE id IN (SELECT row_id FROM given_bodies WHERE place = 'version');
CREATE TRIGGER resource_versions_no_update BEFORE UPDATE ON resource_versions
WHEN NEW.id IS NOT OLD.id OR NEW.resource_id IS NOT OLD.resource_id
  OR (NEW.number IS NOT OLD.number AND OLD.id NOT IN (SELECT version_id FROM shared_publishes))
  OR NEW.body IS NOT OLD.body OR NEW.published_at IS NOT OLD.published_at OR OLD.publisher_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'published resource versions are immutable'); END;
UPDATE resource_drafts
  SET body = (SELECT body FROM given_bodies WHERE place = 'draft' AND row_id = resource_drafts.resource_id)
  WHERE resource_id IN (SELECT row_id FROM given_bodies WHERE place = 'draft');

DROP VIEW temp.given_callable;
DROP VIEW temp.given_admitted;
DROP TABLE temp.given_bodies;
DROP TABLE temp.given_profiles;
DROP TABLE temp.given_steps;
DROP TABLE temp.given_latest;
";
