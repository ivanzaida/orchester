//! *Deny with a message* (ORC-29): the words a denial told the agent, kept with the request it denied. Only a denied
//! request has them, and they never change once written.

pub(super) const MIGRATION: &str = r"
ALTER TABLE tool_approvals ADD COLUMN denial_note TEXT CHECK (denial_note IS NULL OR length(denial_note) > 0);
CREATE TRIGGER denial_notes_denied_only BEFORE UPDATE OF denial_note ON tool_approvals
WHEN NEW.denial_note IS NOT NULL AND NEW.state NOT IN ('dispatching_denial', 'denied')
BEGIN SELECT RAISE(ABORT, 'only a denied request keeps a denial message'); END;
CREATE TRIGGER denial_notes_immutable BEFORE UPDATE OF denial_note ON tool_approvals
WHEN OLD.denial_note IS NOT NULL AND NEW.denial_note IS NOT OLD.denial_note
BEGIN SELECT RAISE(ABORT, 'a denial keeps the message it was given'); END;
";
