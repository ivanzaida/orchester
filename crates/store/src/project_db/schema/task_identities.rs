//! Task identities apart from task definitions (team authority P3, `docs/architecture/team-authority-plan.md` §7). A
//! team run must execute and recover with no current team task row, so the records of a task (runs, conversations,
//! usage, sources, memories, merges, notices, estimates…) refer to its identity in `task_identities`, not to its
//! definition in `tasks`. A local task's identity and definition share the id; a team task's identity holds the
//! server's ids and the revision it was recorded at, never its title, brief, status or configuration.
//!
//! Renaming `tasks` makes SQLite name `task_identities` in every foreign key that named `tasks` (no trigger or view
//! names it); the renamed table then gives its rows to the new identities, definitions and base branch overrides.
//! The parts of a definition itself (its criteria, organization and module) refer to `tasks` again, so they never
//! hang off an identity without a definition. Every row is kept: each task gets an identity, a team one when it is a
//! shared task of the linked server project. A shared row with no link left reads as local, as *Make local* leaves it.

pub(super) const MIGRATION: &str = r"
DROP INDEX tasks_by_batch;
DROP INDEX tasks_by_number;
ALTER TABLE tasks RENAME TO task_identities;

CREATE TABLE task_identities_new (
  id TEXT PRIMARY KEY,
  owner TEXT NOT NULL CHECK (owner IN ('local', 'team')),
  server_id TEXT,
  team_project_id TEXT,
  team_task_id TEXT,
  recorded_revision INTEGER CHECK (recorded_revision > 0),
  CHECK ((owner = 'team') = (server_id IS NOT NULL)),
  CHECK ((owner = 'team') = (team_project_id IS NOT NULL)),
  CHECK ((owner = 'team') = (team_task_id IS NOT NULL)),
  CHECK ((owner = 'team') = (recorded_revision IS NOT NULL)),
  UNIQUE (server_id, team_project_id, team_task_id)
) STRICT;
INSERT INTO task_identities_new (rowid, id, owner, server_id, team_project_id, team_task_id, recorded_revision)
  SELECT task.rowid, task.id, CASE WHEN team.server_id IS NULL THEN 'local' ELSE 'team' END, team.server_id,
    team.project_id, CASE WHEN team.server_id IS NULL THEN NULL ELSE task.id END,
    CASE WHEN team.server_id IS NULL THEN NULL ELSE task.revision END
  FROM task_identities AS task LEFT JOIN sharing AS team ON task.sharing IS 'shared';

CREATE TABLE task_definitions (
  id TEXT PRIMARY KEY REFERENCES task_identities (id),
  number INTEGER NOT NULL CHECK (number > 0),
  batch_key TEXT NOT NULL,
  title TEXT NOT NULL,
  brief TEXT NOT NULL,
  pipeline_id TEXT,
  budget_tokens INTEGER CHECK (budget_tokens > 0),
  max_duration_ms INTEGER CHECK (max_duration_ms > 0),
  status TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  pipeline_version INTEGER CHECK (pipeline_version > 0),
  revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0),
  sharing TEXT CHECK (sharing IN ('shared', 'not_shared')),
  pipeline_slug TEXT,
  former_key TEXT,
  created_by TEXT,
  updated_by TEXT,
  updated_at INTEGER,
  kept_key TEXT,
  kept_tag_names TEXT,
  CHECK ((sharing IS 'not_shared') = (kept_key IS NOT NULL)),
  CHECK ((sharing IS 'not_shared') = (kept_tag_names IS NOT NULL))
) STRICT;
INSERT INTO task_definitions (rowid, id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms,
    status, created_at, pipeline_version, revision, sharing, pipeline_slug, former_key, created_by, updated_by,
    updated_at, kept_key, kept_tag_names)
  SELECT rowid, id, number, batch_key, title, brief, pipeline_id, budget_tokens, max_duration_ms, status, created_at,
    pipeline_version, revision, sharing, pipeline_slug, former_key, created_by, updated_by, updated_at, kept_key,
    kept_tag_names
  FROM task_identities;

-- A task's own base branch (Inherit has no row), kept on this computer for a team task as for a local one.
CREATE TABLE task_bases (
  task_id TEXT PRIMARY KEY REFERENCES task_identities (id),
  choice TEXT NOT NULL CHECK (choice IN ('checked_out', 'branch')),
  branch TEXT CHECK ((choice = 'branch') = (branch IS NOT NULL))
) STRICT;
INSERT INTO task_bases (task_id, choice, branch)
  SELECT id, base_choice, base_branch FROM task_identities WHERE base_choice <> 'inherit';

DROP TABLE task_identities;
ALTER TABLE task_identities_new RENAME TO task_identities;
ALTER TABLE task_definitions RENAME TO tasks;
CREATE INDEX tasks_by_batch ON tasks (batch_key);
CREATE UNIQUE INDEX tasks_by_number ON tasks (number) WHERE sharing IS NOT 'not_shared';

CREATE TRIGGER task_identities_keep_reference BEFORE UPDATE ON task_identities
WHEN NEW.id IS NOT OLD.id OR (OLD.owner = 'team' AND NEW.owner = 'team' AND (NEW.server_id IS NOT OLD.server_id
  OR NEW.team_project_id IS NOT OLD.team_project_id OR NEW.team_task_id IS NOT OLD.team_task_id))
BEGIN SELECT RAISE(ABORT, 'a task identity keeps its id and its team task'); END;
CREATE TRIGGER task_identities_keep_team BEFORE DELETE ON task_identities WHEN OLD.owner = 'team'
BEGIN SELECT RAISE(ABORT, 'a team task identity is never deleted'); END;

CREATE TABLE task_criteria_new (
  task_id TEXT NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  text TEXT NOT NULL,
  PRIMARY KEY (task_id, position)
) STRICT;
INSERT INTO task_criteria_new (task_id, position, text) SELECT task_id, position, text FROM task_criteria;
DROP TABLE task_criteria;
ALTER TABLE task_criteria_new RENAME TO task_criteria;

CREATE TABLE task_organization_new (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;
INSERT INTO task_organization_new (rowid, task_id, body, updated_at)
  SELECT rowid, task_id, body, updated_at FROM task_organization;
DROP TABLE task_organization;
ALTER TABLE task_organization_new RENAME TO task_organization;

DROP INDEX task_modules_by_module;
CREATE TABLE task_modules_new (
  task_id TEXT PRIMARY KEY REFERENCES tasks (id) ON DELETE CASCADE,
  module_id TEXT NOT NULL REFERENCES modules (id)
) STRICT;
INSERT INTO task_modules_new (rowid, task_id, module_id) SELECT rowid, task_id, module_id FROM task_modules;
DROP TABLE task_modules;
ALTER TABLE task_modules_new RENAME TO task_modules;
CREATE INDEX task_modules_by_module ON task_modules (module_id);
";
