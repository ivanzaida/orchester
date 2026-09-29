//! Keep prior attempt history while allowing a conversation to recover a failed run.

pub(super) const MIGRATION: &str = r"
CREATE TABLE attempts_new (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  number INTEGER NOT NULL CHECK (number > 0),
  reason TEXT NOT NULL CHECK (reason IN ('initial', 'retry', 'follow_up', 'recovery')),
  failed_step TEXT,
  return_to TEXT,
  status TEXT NOT NULL CHECK (status IN ('active', 'routed_back', 'succeeded', 'failed', 'cancelled')),
  UNIQUE (run_id, number),
  CHECK ((reason = 'retry') = (failed_step IS NOT NULL)),
  CHECK ((reason = 'initial') = (return_to IS NULL))
) STRICT;
INSERT INTO attempts_new (rowid, id, run_id, number, reason, failed_step, return_to, status)
  SELECT rowid, id, run_id, number, reason, failed_step, return_to, status FROM attempts;
DROP TABLE attempts;
ALTER TABLE attempts_new RENAME TO attempts;
ALTER TABLE messages ADD COLUMN recovery_sequence INTEGER
  CHECK (recovery_sequence IS NULL OR (recovery_sequence > 0 AND run_id IS NOT NULL AND author = 'person'));
CREATE TRIGGER messages_recovery_immutable BEFORE UPDATE OF recovery_sequence ON messages
WHEN NEW.recovery_sequence IS NOT OLD.recovery_sequence
BEGIN
  SELECT RAISE(ABORT, 'a message recovery request is immutable');
END;
";
