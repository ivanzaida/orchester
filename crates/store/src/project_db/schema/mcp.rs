//! MCP servers of the project (Connections › MCP servers).

pub(super) const MIGRATION: &str = r"
-- MCP servers the project's agent profiles can give their agents. `settings` holds the transport and
-- the variables as JSON; a variable taken from a secret names the secret's id, never its value.
-- Names are the key the agent CLIs know a server by, unique within the project.
CREATE TABLE mcp_servers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  settings TEXT NOT NULL,
  enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
  created_at INTEGER NOT NULL
) STRICT;
";
