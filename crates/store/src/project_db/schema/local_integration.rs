pub(super) const MIGRATION: &str = r"
CREATE TABLE local_integrations (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  version INTEGER NOT NULL CHECK (version >= 0),
  body TEXT NOT NULL
) STRICT;
CREATE TABLE local_recovery_content (
  run_id TEXT NOT NULL REFERENCES local_integrations (run_id),
  path TEXT NOT NULL,
  layer TEXT NOT NULL CHECK (layer IN ('head', 'staged', 'working', 'task_ancestor', 'task_current', 'task_incoming', 'working_base', 'result')),
  digest TEXT NOT NULL,
  content BLOB NOT NULL,
  PRIMARY KEY (run_id, path, layer)
) STRICT;
CREATE TABLE local_integration_process (
  run_id TEXT PRIMARY KEY REFERENCES local_integrations (run_id),
  sequence INTEGER NOT NULL,
  identity TEXT,
  finished INTEGER NOT NULL CHECK (finished IN (0, 1))
) STRICT;
";
