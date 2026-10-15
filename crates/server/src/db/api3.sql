-- API 3 (docs/architecture/server.md, "Migration of S1a accounts"): no passwords, device tokens issued by cloud
-- joins and codes, invites, the cloud link, join approval. Runs with foreign keys off, after `projects` is rebuilt
-- and the sessions limited to a password change are revoked.
ALTER TABLE sessions DROP COLUMN initial_password;
ALTER TABLE sessions ADD COLUMN via TEXT NOT NULL DEFAULT 'before_3' CHECK (via IN ('cloud','code','before_3'));
ALTER TABLE sessions ADD COLUMN kid TEXT;
ALTER TABLE users DROP COLUMN password_hash;
ALTER TABLE users DROP COLUMN must_change_password;
ALTER TABLE users DROP COLUMN password_changed_at;
ALTER TABLE users ADD COLUMN cloud_subject TEXT;
CREATE UNIQUE INDEX users_cloud_subject ON users(cloud_subject) WHERE cloud_subject IS NOT NULL;
CREATE TABLE server_keys (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  server_id TEXT NOT NULL,
  signing_key BLOB NOT NULL,
  code_key BLOB NOT NULL CHECK (length(code_key) = 32)
);
CREATE TABLE settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  join_approval INTEGER NOT NULL CHECK (join_approval IN (0,1))
);
INSERT INTO settings VALUES (1, 0);
CREATE TABLE invites (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('server','project','account')),
  delivery TEXT NOT NULL CHECK (delivery IN ('cloud','code')),
  email TEXT,
  project TEXT REFERENCES projects(id),
  role TEXT CHECK (role IN ('owner','member')),
  user_id TEXT REFERENCES users(id),
  admin INTEGER NOT NULL CHECK (admin IN (0,1)),
  made_by TEXT NOT NULL REFERENCES users(id),
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  accepted_at INTEGER,
  accepted_by TEXT REFERENCES users(id),
  revoked_at INTEGER
);
CREATE TABLE codes (
  hmac BLOB NOT NULL CHECK (length(hmac) = 32),
  kind TEXT NOT NULL CHECK (kind IN ('setup','invite','account','pairing')),
  invite TEXT REFERENCES invites(id),
  user_id TEXT REFERENCES users(id),
  made_by TEXT REFERENCES users(id),
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  used_at INTEGER
);
CREATE TABLE cloud_link (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  root_key TEXT NOT NULL,
  key_set TEXT,
  key_set_issued_at INTEGER,
  key_set_expires_at INTEGER,
  accepting INTEGER NOT NULL CHECK (accepting IN (0,1)),
  linked_at INTEGER NOT NULL
);
CREATE TABLE used_identity_tokens (jti TEXT PRIMARY KEY, expires_at INTEGER NOT NULL);
CREATE TABLE approvals (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  cloud_subject TEXT NOT NULL,
  device TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('waiting','approved','refused','used')),
  requested_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  decided_by TEXT REFERENCES users(id),
  decided_at INTEGER
);
UPDATE meta SET schema_version=3;
