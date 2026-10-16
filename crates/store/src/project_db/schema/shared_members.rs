//! The members of a shared project as the server last listed them (`docs/architecture/server.md`, row 63: members
//! are read-only in the app), so Sharing shows them while the server cannot be reached, as of when they were read.

pub(super) const MIGRATION: &str = r"
CREATE TABLE shared_members (
  user_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  username TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('owner', 'member')),
  added_at INTEGER NOT NULL
) STRICT;

-- When the members were last read from the server; NULL until they have been.
ALTER TABLE sharing ADD COLUMN members_read_at INTEGER;
";
