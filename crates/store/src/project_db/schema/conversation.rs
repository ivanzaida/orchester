//! The task conversation and follow-ups (M8).

pub(super) const MIGRATION: &str = r"
-- The task conversation (M8). A person's message is queued for the task's run (NULL before the task
-- had one), delivered with an agent invocation (`step_execution_id`, `invocation_id`) and
-- acknowledged once that agent ended a turn after reading it. An agent's message is a reply or a
-- question, written with the invocation it ended. What a message says never changes, a message is
-- never deleted, and its delivery only moves forward.
CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  run_id TEXT REFERENCES runs (id),
  idempotency_key TEXT UNIQUE,
  author TEXT NOT NULL CHECK (author IN ('person', 'agent')),
  kind TEXT CHECK (kind IN ('reply', 'question')),
  recipient_profile TEXT,
  step_execution_id TEXT REFERENCES step_executions (id),
  invocation_id TEXT REFERENCES invocations (id),
  body TEXT NOT NULL CHECK (length(trim(body)) > 0),
  created_at INTEGER NOT NULL,
  delivered_at INTEGER,
  acknowledged_at INTEGER,
  CHECK ((author = 'agent') = (kind IS NOT NULL)),
  CHECK ((author = 'person') = (idempotency_key IS NOT NULL)),
  CHECK (author = 'person' OR (run_id IS NOT NULL AND step_execution_id IS NOT NULL AND invocation_id IS NOT NULL
    AND recipient_profile IS NULL AND delivered_at IS NULL AND acknowledged_at IS NULL)),
  CHECK (author = 'agent' OR ((delivered_at IS NULL) = (invocation_id IS NULL)
    AND (invocation_id IS NULL) = (step_execution_id IS NULL)
    AND (delivered_at IS NOT NULL OR acknowledged_at IS NULL)
    AND (delivered_at IS NULL OR run_id IS NOT NULL)))
) STRICT;
CREATE INDEX messages_by_task ON messages (task_id, created_at);
CREATE TRIGGER messages_move_forward BEFORE UPDATE ON messages
WHEN NEW.id IS NOT OLD.id OR NEW.task_id IS NOT OLD.task_id OR NEW.idempotency_key IS NOT OLD.idempotency_key
  OR NEW.author IS NOT OLD.author OR NEW.kind IS NOT OLD.kind OR NEW.recipient_profile IS NOT OLD.recipient_profile
  OR NEW.body IS NOT OLD.body OR NEW.created_at IS NOT OLD.created_at
  OR (OLD.run_id IS NOT NULL AND NEW.run_id IS NOT OLD.run_id)
  OR (OLD.delivered_at IS NOT NULL AND (NEW.delivered_at IS NOT OLD.delivered_at
    OR NEW.step_execution_id IS NOT OLD.step_execution_id OR NEW.invocation_id IS NOT OLD.invocation_id))
  OR (OLD.acknowledged_at IS NOT NULL AND NEW.acknowledged_at IS NOT OLD.acknowledged_at)
BEGIN SELECT RAISE(ABORT, 'a message keeps what it says, and its delivery only moves forward'); END;
CREATE TRIGGER messages_no_delete BEFORE DELETE ON messages
BEGIN SELECT RAISE(ABORT, 'messages are never deleted'); END;

-- A follow-up (Ask agent after a run succeeded) is an attempt that executes one step, which
-- `return_to` names. SQLite cannot change a CHECK constraint, so the table is rebuilt, its rows
-- keeping their `rowid`.
CREATE TABLE attempts_new (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  number INTEGER NOT NULL CHECK (number > 0),
  reason TEXT NOT NULL CHECK (reason IN ('initial', 'retry', 'follow_up')),
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
";
