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
