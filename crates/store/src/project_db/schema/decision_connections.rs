pub(super) const MIGRATION: &str = r"
CREATE TABLE decision_connections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  settings TEXT NOT NULL
) STRICT;

-- Separate from agent connections: an agent profile can never select this connection.
CREATE TABLE decision_reviewer_connection (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  connection_id TEXT NOT NULL REFERENCES decision_connections (id)
) STRICT;
";
