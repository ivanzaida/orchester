//! Retain raw unfinished editor candidates separately from applied resolution content.
pub(super) const MIGRATION: &str = r"
CREATE TABLE local_recovery_content_with_drafts (
  run_id TEXT NOT NULL REFERENCES local_integrations (run_id),
  path TEXT NOT NULL,
  layer TEXT NOT NULL CHECK (layer IN ('head', 'staged', 'working', 'task_ancestor', 'task_current', 'task_incoming', 'working_base', 'result', 'editor_draft')),
  digest TEXT NOT NULL,
  content BLOB NOT NULL,
  PRIMARY KEY (run_id, path, layer)
) STRICT;
INSERT INTO local_recovery_content_with_drafts SELECT run_id, path, layer, digest, content FROM local_recovery_content;
DROP TABLE local_recovery_content;
ALTER TABLE local_recovery_content_with_drafts RENAME TO local_recovery_content;
";
