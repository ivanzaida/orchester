//! A run's agent events by kind (ORC-914). Reading one run's turns by kind (what a task took, the latest message of a
//! step) had only `agent_events_by_kind (kind, at)` to go by, which SQLite walked over every run's rows of that kind:
//! 30 s on a project with a million events, 8 to 76 s while a merge held the write lock.

pub(super) const MIGRATION: &str = r"
CREATE INDEX agent_events_by_run_kind ON agent_events (run_id, kind, sequence);
";
