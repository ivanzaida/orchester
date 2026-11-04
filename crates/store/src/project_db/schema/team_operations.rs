//! Team operations (team authority P2, `docs/architecture/team-authority-plan.md` §4, "Results and failure recovery"):
//! the operation id and the request of every team command, recorded before it is sent, so a lost response is looked up
//! or sent again with the same id and the same request after a restart, and never becomes a second task, publication
//! or run. The request is the command as the server reads it, without credentials; `digest` is its SHA-256, which a
//! retry under the same id must match. A committed command keeps its receipt, the local effects it allows; a refused
//! one its refusal. A settled operation never changes again, and no operation is deleted while the project lasts.

pub(super) const MIGRATION: &str = r"
CREATE TABLE team_operations (
  id TEXT PRIMARY KEY,
  server_id TEXT NOT NULL,
  team_project_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  digest TEXT NOT NULL CHECK (length(digest) = 64),
  request TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('sent', 'committed', 'completed', 'effect_failed', 'rejected')),
  receipt TEXT CHECK ((state IN ('committed', 'completed', 'effect_failed')) = (receipt IS NOT NULL)),
  rejection TEXT CHECK ((state = 'rejected') = (rejection IS NOT NULL)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;
CREATE INDEX team_operations_open ON team_operations (created_at) WHERE state IN ('sent', 'committed', 'effect_failed');

CREATE TRIGGER team_operations_keep_request BEFORE UPDATE ON team_operations
WHEN NEW.id IS NOT OLD.id OR NEW.server_id IS NOT OLD.server_id OR NEW.team_project_id IS NOT OLD.team_project_id
  OR NEW.kind IS NOT OLD.kind OR NEW.digest IS NOT OLD.digest OR NEW.request IS NOT OLD.request
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'a team operation keeps its request'); END;
CREATE TRIGGER team_operations_settled_once BEFORE UPDATE ON team_operations
WHEN OLD.state IN ('completed', 'rejected')
  OR (OLD.state IN ('committed', 'effect_failed') AND (NEW.state IN ('sent', 'rejected') OR NEW.receipt IS NOT OLD.receipt))
BEGIN SELECT RAISE(ABORT, 'a settled team operation keeps its outcome'); END;
CREATE TRIGGER team_operations_no_delete BEFORE DELETE ON team_operations
BEGIN SELECT RAISE(ABORT, 'team operations are never deleted'); END;
";
