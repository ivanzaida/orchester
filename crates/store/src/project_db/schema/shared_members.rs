//! The members of a shared project as the server last listed them (`docs/architecture/server.md`, row 63: members
//! are read-only in the app), and the repository it was set up for, so Sharing shows them while the server cannot be
//! reached, as of when they were read.

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

-- The repository the server project was set up for (NULL: it names none), as last read, and when; NULL until read.
ALTER TABLE sharing ADD COLUMN remote_hint TEXT;
ALTER TABLE sharing ADD COLUMN remote_hint_read_at INTEGER;
";

/// Who this computer was signed in as when it read the members, so the list still names the person once it signed out
/// of the server and no longer knows its account there.
pub(super) const READ_AS_MIGRATION: &str = r"
-- The person's user id on the server when the members were last read; NULL when the reader was not one of them.
ALTER TABLE sharing ADD COLUMN members_read_as TEXT;
";

/// The server's admins as the members list last named them: they change the team's configuration as the project's
/// owners do (row 72), so a member's libraries name them.
pub(super) const SERVER_ADMINS_MIGRATION: &str = r"
CREATE TABLE shared_server_admins (
  user_id TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  username TEXT NOT NULL
) STRICT;
";
