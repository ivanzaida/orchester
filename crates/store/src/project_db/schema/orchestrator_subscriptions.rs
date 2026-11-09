//! What the project's Orchestrator conversations watch (ORC-758, `orchester_core::orchestrator::watch`).

pub(super) const MIGRATION: &str = r"
-- A conversation's subscription to a target: a task or module (target_id names it), the tasks without a module or the
-- whole project (no id), with the person's order (empty: the project's default) and since when it leads. An ended one
-- keeps when and why it ended for its conversation's lines and History: by itself (task_done, task_canceled,
-- module_closed with how many tasks were done and canceled), stopped by the person, taken over by another
-- conversation (taken_by), renewed by its own with a new order, or its conversation deleted. The conversation may be
-- gone, so chat_id refers to nothing.
CREATE TABLE orchestrator_subscriptions (
  id INTEGER PRIMARY KEY,
  chat_id TEXT NOT NULL,
  target_kind TEXT NOT NULL CHECK (target_kind IN ('task', 'module', 'without_module', 'project')),
  target_id TEXT CHECK ((target_id IS NOT NULL) = (target_kind IN ('task', 'module'))),
  order_text TEXT NOT NULL,
  since INTEGER NOT NULL,
  ended_at INTEGER,
  end_reason TEXT CHECK (end_reason IN
    ('task_done', 'task_canceled', 'module_closed', 'stopped', 'taken_over', 'renewed', 'chat_deleted')),
  taken_by TEXT,
  module_done INTEGER CHECK (module_done >= 0),
  module_canceled INTEGER CHECK (module_canceled >= 0),
  CHECK ((ended_at IS NULL) = (end_reason IS NULL)),
  CHECK ((taken_by IS NOT NULL) = (end_reason IS 'taken_over')),
  CHECK ((module_done IS NOT NULL) = (end_reason IS 'module_closed')),
  CHECK ((module_canceled IS NOT NULL) = (end_reason IS 'module_closed'))
) STRICT;
-- One conversation leads a target at most: two subscriptions that have not ended never name the same target, whichever
-- conversations write them at once.
CREATE UNIQUE INDEX orchestrator_subscriptions_leader
  ON orchestrator_subscriptions (target_kind, IFNULL(target_id, '')) WHERE ended_at IS NULL;
CREATE INDEX orchestrator_subscriptions_chat ON orchestrator_subscriptions (chat_id, id);
";
