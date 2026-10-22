-- Schema 7, modules (docs/architecture/server.md, "Modules"): a project's modules under a revision, the module each task
-- is in, every move of a task into or out of one (append-only, for each desktop's burndown), and the first answer to a
-- module create or change by its idempotency key. `events` is rebuilt by the migration to take the `modules` kind.
CREATE TABLE modules (
  id TEXT PRIMARY KEY, project TEXT NOT NULL REFERENCES projects(id), name TEXT NOT NULL, normalized TEXT NOT NULL,
  description TEXT NOT NULL, start_date TEXT, target_date TEXT, state TEXT NOT NULL CHECK(state IN ('open','closed')),
  revision INTEGER NOT NULL CHECK(revision>0),
  created_by TEXT NOT NULL REFERENCES users(id), created_at INTEGER NOT NULL,
  updated_by TEXT NOT NULL REFERENCES users(id), updated_at INTEGER NOT NULL,
  UNIQUE(project,normalized)
);
ALTER TABLE tasks ADD COLUMN module TEXT REFERENCES modules(id);
CREATE TABLE module_changes (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT, project TEXT NOT NULL REFERENCES projects(id),
  task TEXT NOT NULL REFERENCES tasks(id), from_module TEXT REFERENCES modules(id), to_module TEXT REFERENCES modules(id),
  user_id TEXT NOT NULL REFERENCES users(id), actor TEXT NOT NULL CHECK(actor IN ('person','orchester')),
  at INTEGER NOT NULL, CHECK(from_module IS NOT to_module)
);
CREATE INDEX module_changes_project ON module_changes(project,sequence);
CREATE TRIGGER module_changes_no_update BEFORE UPDATE ON module_changes BEGIN SELECT RAISE(ABORT,'module history is append-only'); END;
CREATE TRIGGER module_changes_no_delete BEFORE DELETE ON module_changes BEGIN SELECT RAISE(ABORT,'module history is append-only'); END;
CREATE TABLE change_keys (
  project TEXT NOT NULL REFERENCES projects(id), user_id TEXT NOT NULL REFERENCES users(id), key TEXT NOT NULL,
  response TEXT NOT NULL, at INTEGER NOT NULL, PRIMARY KEY(project,user_id,key)
);
CREATE INDEX change_keys_retention ON change_keys(at);
CREATE TABLE events_7 (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT, project TEXT NOT NULL REFERENCES projects(id),
  kind TEXT NOT NULL CHECK(kind IN ('task','members','tags','project','modules')), payload TEXT NOT NULL,
  at INTEGER NOT NULL
);
INSERT INTO events_7(sequence,project,kind,payload,at) SELECT sequence,project,kind,payload,at FROM events;
DROP TABLE events;
ALTER TABLE events_7 RENAME TO events;
CREATE INDEX events_project ON events(project,sequence);
CREATE INDEX events_retention ON events(at);
CREATE INDEX events_project_retention ON events(project,at);
CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'events are append-only'); END;
UPDATE meta SET schema_version=7;
