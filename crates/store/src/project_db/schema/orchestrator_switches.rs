//! The Orchestrator's six switches, each project's own (ORC-593, `docs/design/settings.md`, the person's decision 2 of
//! 2026-11-01): Project settings › Orchestrator, moved from the application database's `app_settings`.

pub(super) const MIGRATION: &str = r"
-- What the Orchestrator chat may do itself in this project. NULL: never set for the project, so the workspace can
-- tell it from a person's choice and take the application-wide value it moved from.
CREATE TABLE orchestrator_switches (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  publishes_drafts INTEGER CHECK (publishes_drafts IN (0, 1)),
  answers_agents INTEGER CHECK (answers_agents IN (0, 1)),
  merges_runs INTEGER CHECK (merges_runs IN (0, 1)),
  merges_finished INTEGER CHECK (merges_finished IN (0, 1)),
  continues_interrupted INTEGER CHECK (continues_interrupted IN (0, 1)),
  writes_git INTEGER CHECK (writes_git IN (0, 1))
) STRICT;
INSERT INTO orchestrator_switches (id) VALUES (1);
";

/// The six switches became one (ORC-909): a project that let the Orchestrator answer agents lets it drive, one that
/// did not (or never said) does not, and takes the application-wide value once it is read. The six old columns stay in
/// the table unread: SQLite cannot drop a column a CHECK names without rebuilding the table.
pub(super) const DRIVE_MIGRATION: &str = r"
ALTER TABLE orchestrator_switches ADD COLUMN drives INTEGER CHECK (drives IN (0, 1));
UPDATE orchestrator_switches SET drives = answers_agents WHERE id = 1;
";
