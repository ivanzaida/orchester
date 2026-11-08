//! What this computer fills in per project for the team's configuration (`docs/design/team-configuration.md`, "What a
//! computer fills in"): the connection and account picked for a team profile, which team MCP servers are on here, and
//! the keychain handle of each secret value by the secret's name. Resource ids are the team server's; nothing here
//! points at a row of `resources`, so a connection or resource that goes away leaves a pick that reads as missing.

pub(super) const MIGRATION: &str = r"
CREATE TABLE team_connection_picks (
  resource_id   TEXT PRIMARY KEY,
  connection_id TEXT NOT NULL,
  account_id    TEXT
) STRICT;

CREATE TABLE team_mcp_switches (
  resource_id TEXT PRIMARY KEY,
  enabled     INTEGER NOT NULL CHECK (enabled IN (0, 1))
) STRICT;

-- The value is only in the OS keychain, under the handle; the row never holds it.
CREATE TABLE team_secret_values (
  name   TEXT PRIMARY KEY,
  handle TEXT NOT NULL UNIQUE
) STRICT;
";
