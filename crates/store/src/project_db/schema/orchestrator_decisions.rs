//! What the Orchestrator decided on its own (`orchester_core::orchestrator::decision`,
//! `docs/plan/orchestrator-autonomy/OA-22-…`).

pub(super) const MIGRATION: &str = r"
-- A decision the Orchestrator made in a turn nobody asked for: the conversation whose turn made it, the tool, the
-- task (its key as people read it) and the run it named, its reason in the chat's words, what undoes it when
-- something does, and whether the person looked at it (open, seen or overruled, and when).
CREATE TABLE orchestrator_decisions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chat_id TEXT NOT NULL,
  tool TEXT NOT NULL,
  task_key TEXT,
  run_id TEXT,
  reason TEXT NOT NULL,
  undo TEXT,
  at INTEGER NOT NULL,
  review TEXT NOT NULL DEFAULT 'open',
  reviewed_at INTEGER
) STRICT;
CREATE INDEX orchestrator_decisions_by_review ON orchestrator_decisions (review, at);
";
