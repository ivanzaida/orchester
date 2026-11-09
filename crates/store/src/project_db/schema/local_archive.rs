//! Local MCP servers that were archived (ORC-842): an archived server keeps its row and settings, so what pins it still
//! reads them and it can be restored as it was, but no list or picker offers it. A local project's other resources are
//! archived in `removed_resources`.

pub(super) const MIGRATION: &str = r"
CREATE TABLE archived_mcp_servers (
  server_id TEXT PRIMARY KEY REFERENCES mcp_servers (id) ON DELETE CASCADE,
  archived_at INTEGER NOT NULL
) STRICT;

-- The runs an MCP server's archive stopped: the stop itself is in `archive_stops`, which names resources only.
CREATE TABLE mcp_server_stops (
  run_id TEXT PRIMARY KEY REFERENCES archive_stops (run_id),
  server_id TEXT NOT NULL REFERENCES mcp_servers (id) ON DELETE CASCADE
) STRICT;
";
