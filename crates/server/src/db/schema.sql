CREATE TABLE meta (schema_version INTEGER NOT NULL);
INSERT INTO meta VALUES (1);
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  username TEXT NOT NULL COLLATE NOCASE UNIQUE,
  display_name TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  admin INTEGER NOT NULL CHECK (admin IN (0,1)),
  disabled_at INTEGER,
  must_change_password INTEGER NOT NULL CHECK (must_change_password IN (0,1)),
  created_at INTEGER NOT NULL,
  password_changed_at INTEGER NOT NULL
);
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  token_hash BLOB NOT NULL UNIQUE CHECK (length(token_hash) = 32),
  device TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_used_at INTEGER NOT NULL,
  revoked_at INTEGER,
  initial_password INTEGER NOT NULL CHECK (initial_password IN (0,1))
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE TABLE audit_events (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  actor TEXT,
  action TEXT NOT NULL,
  target TEXT,
  address TEXT,
  at INTEGER NOT NULL
);
CREATE TRIGGER audit_no_update BEFORE UPDATE ON audit_events BEGIN
  SELECT RAISE(ABORT, 'audit records are append-only');
END;
CREATE TRIGGER audit_no_delete BEFORE DELETE ON audit_events BEGIN
  SELECT RAISE(ABORT, 'audit records are append-only');
END;
