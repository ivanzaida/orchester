//! Messages sent now to an agent at work (ORC-61): when a person's message reaches its agent, how it
//! arrived, and a message sent now that the agent's CLI did not take.

pub(super) const MIGRATION: &str = r"
-- When a person's message reaches an agent at work: after its turn, or now, mid-turn.
ALTER TABLE messages ADD COLUMN timing TEXT NOT NULL DEFAULT 'after_turn'
  CHECK (timing IN ('after_turn', 'now') AND (timing = 'after_turn' OR author = 'person'));
-- How a delivered message reached its agent; NULL for one delivered before this was kept, which came
-- with an invocation's prompt.
ALTER TABLE messages ADD COLUMN arrival TEXT
  CHECK (arrival IS NULL OR (arrival IN ('prompt', 'mid_turn', 'turn_end') AND delivered_at IS NOT NULL));
-- A message sent now that the CLI of `undelivered_execution`'s agent did not take, and why. It goes
-- no further unless the person sends it after the turn, which queues it again.
ALTER TABLE messages ADD COLUMN undelivered_execution TEXT REFERENCES step_executions (id);
ALTER TABLE messages ADD COLUMN undelivered_at INTEGER;
ALTER TABLE messages ADD COLUMN undelivered_reason TEXT
  CHECK ((undelivered_reason IS NULL) = (undelivered_at IS NULL)
    AND (undelivered_reason IS NULL) = (undelivered_execution IS NULL)
    AND (undelivered_reason IS NULL OR (undelivered_reason IN ('refused', 'unwritable')
      AND delivered_at IS NULL AND run_id IS NOT NULL)));
CREATE TRIGGER messages_timing_only_after_turn BEFORE UPDATE OF timing ON messages
WHEN NEW.timing IS NOT OLD.timing AND NOT (OLD.timing = 'now' AND NEW.timing = 'after_turn')
BEGIN SELECT RAISE(ABORT, 'a message sent now may only be sent after the turn instead'); END;
";
