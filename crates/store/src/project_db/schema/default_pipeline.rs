//! The project's default pipeline (ORC-106), which New task starts with.

pub(super) const MIGRATION: &str = r"
-- Pipelines › a published row's Make default: the pipeline New task chooses at first. Without a row there is none.
CREATE TABLE project_default_pipeline (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  pipeline_id TEXT NOT NULL REFERENCES resources (id)
) STRICT;
";
