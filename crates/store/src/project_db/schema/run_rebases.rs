//! Runs whose branch git proved rebased onto a newer commit of the branch they merge into after they
//! started (`orchester_core::integration::local::RunRebase`): the base their own commits sit on now,
//! and the branch tip that stands for a last checkpoint the rebase rewrote. One row per run; a newer
//! rebase replaces it.

pub(super) const MIGRATION: &str = r"
CREATE TABLE run_rebases (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  base_revision TEXT NOT NULL,
  replaced_checkpoint TEXT,
  checkpoint_tip TEXT,
  recorded_at INTEGER NOT NULL,
  CHECK ((replaced_checkpoint IS NULL) = (checkpoint_tip IS NULL))
) STRICT;
";
