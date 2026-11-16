//! The tasks the Orchestrator parked for the person (`orchester_core::orchestrator::parking`,
//! `docs/plan/orchestrator-autonomy/OA-15-…`).

pub(super) const MIGRATION: &str = r"
-- A task parked for the person: the run that waited then and the status it had (NULL: the task had no run), what
-- kind of thing is needed, why the chat could not go on and what to do, in its words, and the conversation whose
-- turn parked it. One row per task; gone once the parking ends.
CREATE TABLE parked_tasks (
  task_id TEXT PRIMARY KEY,
  run_id TEXT,
  run_status TEXT,
  needs TEXT NOT NULL,
  why TEXT NOT NULL,
  to_do TEXT NOT NULL,
  chat_id TEXT NOT NULL,
  at INTEGER NOT NULL
) STRICT;
";

pub(super) const STANDINGS_MIGRATION: &str = r"
-- The state last seen of what a subscription leads (`orchester_core::orchestrator::standing::Raised::state`, or
-- 'quiet'), by the conversation, the target and since when it leads: an event is raised when it changes.
CREATE TABLE orchestrator_standings (
  chat_id TEXT NOT NULL,
  target TEXT NOT NULL,
  since INTEGER NOT NULL,
  state TEXT NOT NULL,
  PRIMARY KEY (chat_id, target, since)
) STRICT;
";
