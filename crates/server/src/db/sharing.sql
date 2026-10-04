CREATE TABLE projects (
  id TEXT PRIMARY KEY, key TEXT NOT NULL, name TEXT NOT NULL, remote TEXT NOT NULL UNIQUE,
  next_task_number INTEGER NOT NULL CHECK(next_task_number BETWEEN 1 AND 4294967296),
  created_by TEXT NOT NULL REFERENCES users(id), created_at INTEGER NOT NULL,
  event_floor INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE project_batches (
  user_id TEXT NOT NULL REFERENCES users(id), key TEXT NOT NULL, response TEXT NOT NULL,
  PRIMARY KEY(user_id,key)
);
CREATE TABLE memberships (
  project TEXT NOT NULL REFERENCES projects(id), user_id TEXT NOT NULL REFERENCES users(id),
  role TEXT NOT NULL CHECK(role IN ('owner','member')), added_by TEXT NOT NULL REFERENCES users(id), added_at INTEGER NOT NULL,
  generation TEXT NOT NULL,
  PRIMARY KEY(project,user_id)
);
CREATE TABLE tasks (
  id TEXT PRIMARY KEY, project TEXT NOT NULL REFERENCES projects(id), number INTEGER NOT NULL CHECK(number>0),
  former_key TEXT, title TEXT NOT NULL, brief TEXT NOT NULL, criteria TEXT NOT NULL, pipeline TEXT,
  budget_tokens TEXT, max_duration_ms TEXT, status TEXT NOT NULL CHECK(status IN ('draft','queued')),
  workflow TEXT NOT NULL, priority TEXT NOT NULL, blocked_reason TEXT,
  revision INTEGER NOT NULL CHECK(revision>0), has_run INTEGER NOT NULL CHECK(has_run IN (0,1)),
  created_by TEXT NOT NULL REFERENCES users(id), created_at INTEGER NOT NULL,
  updated_by TEXT NOT NULL REFERENCES users(id), updated_at INTEGER NOT NULL,
  UNIQUE(project,number)
);
CREATE TABLE task_batches (
  project TEXT NOT NULL REFERENCES projects(id), key TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id),
  response TEXT NOT NULL, PRIMARY KEY(project,key)
);
CREATE TABLE tags (
  id TEXT PRIMARY KEY, project TEXT NOT NULL REFERENCES projects(id), name TEXT NOT NULL, normalized TEXT NOT NULL,
  UNIQUE(project,normalized)
);
CREATE TABLE task_tags (task TEXT NOT NULL REFERENCES tasks(id), tag TEXT NOT NULL REFERENCES tags(id), PRIMARY KEY(task,tag));
CREATE TABLE task_links (task TEXT NOT NULL REFERENCES tasks(id), blocking TEXT NOT NULL REFERENCES tasks(id), gate TEXT NOT NULL, PRIMARY KEY(task,blocking));
CREATE TABLE organization_changes (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT, task TEXT NOT NULL REFERENCES tasks(id),
  user_id TEXT NOT NULL REFERENCES users(id), actor TEXT NOT NULL, at INTEGER NOT NULL,
  before TEXT NOT NULL, after TEXT NOT NULL, from_tag_names TEXT NOT NULL, to_tag_names TEXT NOT NULL
);
CREATE TRIGGER organization_no_update BEFORE UPDATE ON organization_changes BEGIN SELECT RAISE(ABORT,'history is append-only'); END;
CREATE TRIGGER organization_no_delete BEFORE DELETE ON organization_changes BEGIN SELECT RAISE(ABORT,'history is append-only'); END;
CREATE TABLE claims (
  id TEXT NOT NULL UNIQUE, task TEXT PRIMARY KEY REFERENCES tasks(id), holder TEXT NOT NULL REFERENCES users(id),
  kind TEXT NOT NULL CHECK(kind IN ('manual','run')), since INTEGER NOT NULL, heartbeat_at INTEGER NOT NULL,
  taken_over_from TEXT REFERENCES users(id)
);
CREATE TABLE claim_events (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT, task TEXT NOT NULL REFERENCES tasks(id), claim TEXT NOT NULL,
  action TEXT NOT NULL, user_id TEXT NOT NULL REFERENCES users(id), from_user TEXT REFERENCES users(id), at INTEGER NOT NULL
);
CREATE TRIGGER claim_no_update BEFORE UPDATE ON claim_events BEGIN SELECT RAISE(ABORT,'claim history is append-only'); END;
CREATE TRIGGER claim_no_delete BEFORE DELETE ON claim_events BEGIN SELECT RAISE(ABORT,'claim history is append-only'); END;
CREATE TABLE run_status (
  task TEXT PRIMARY KEY REFERENCES tasks(id), holder TEXT NOT NULL REFERENCES users(id), number INTEGER NOT NULL,
  phase TEXT NOT NULL, step TEXT, started_at INTEGER NOT NULL, phase_since INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE events (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT, project TEXT NOT NULL REFERENCES projects(id),
  kind TEXT NOT NULL CHECK(kind IN ('task','members','tags','project')), payload TEXT NOT NULL, at INTEGER NOT NULL
);
CREATE INDEX events_project ON events(project,sequence);
CREATE INDEX events_retention ON events(at);
CREATE INDEX events_project_retention ON events(project,at);
CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'events are append-only'); END;
UPDATE meta SET schema_version=2;
