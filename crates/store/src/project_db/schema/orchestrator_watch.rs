//! How the Orchestrator watches the project's runs (ORC-757, `orchester_core::orchestrator::WatchSettings`).

pub(super) const MIGRATION: &str = r"
-- Project settings › Orchestrator, one row: the order a /watch command without text takes (empty: none) and after how
-- many minutes without progress a run is reported (NULL: never). Every project starts with no order and 20 minutes.
CREATE TABLE orchestrator_watch (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  default_order TEXT NOT NULL,
  no_progress_minutes INTEGER CHECK (no_progress_minutes IS NULL OR no_progress_minutes BETWEEN 1 AND 240)
) STRICT;
INSERT INTO orchestrator_watch (id, default_order, no_progress_minutes) VALUES (1, '', 20);
";

/// The ceilings a new subscription that leads to the end takes (Project settings › Orchestrator › *Leading to the
/// end*, `docs/plan/orchestrator-autonomy/OA-10-…`).
pub(super) const LEAD_DEFAULTS_MIGRATION: &str = r"
-- The project's ceilings for a new leading subscription, as JSON (`orchester_core::orchestrator::watch::Ceilings`);
-- NULL while the project never set them, which reads as the built-in ones: 3 runs of a task, 4 returns of a run, 1
-- raise by 50 % and no budget. A subscription keeps the ceilings it started with (orchestrator_subscriptions.lead).
ALTER TABLE orchestrator_watch ADD COLUMN lead_ceilings TEXT;
";
