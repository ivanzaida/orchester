//! Usage is split by the agent provider it ran on (decided 2026-10-28): `invocation_connections` records the agent
//! connection an agent invocation started on with its name then, and `reviewer_usage` the connection a review ran on,
//! so that their usage still names a provider renamed or gone since. Rows from before stay without one; an invocation
//! then reads as on its step's connection, a review as Earlier runs.

pub(super) const MIGRATION: &str = r"
CREATE TABLE invocation_connections (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  connection_id TEXT NOT NULL,
  name TEXT NOT NULL
) STRICT;
CREATE TRIGGER invocation_connections_immutable BEFORE UPDATE ON invocation_connections
BEGIN SELECT RAISE(ABORT, 'an invocation keeps the connection it started on'); END;

ALTER TABLE reviewer_usage ADD COLUMN connection_id TEXT;
ALTER TABLE reviewer_usage ADD COLUMN connection_name TEXT
  CHECK ((connection_id IS NULL) = (connection_name IS NULL));
";
