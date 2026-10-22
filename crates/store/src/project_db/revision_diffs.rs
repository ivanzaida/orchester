//! `revision_diffs`: the diff between two commits, its text in a blob.

use orchester_core::{diff::RevisionDiff, revision::Revision, store::RevisionDiffStore};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb, blobs,
  pull_request_diffs::{DIFF_MEDIA_TYPE, StoredDiff},
};
use crate::StoreError;

impl RevisionDiffStore for ProjectDb {
  type Error = StoreError;

  fn save_revision_diff(&mut self, diff: &RevisionDiff) -> Result<(), StoreError> {
    let blob = blobs::write(&self.directory, diff.text.as_bytes())?;
    let transaction = self.connection.transaction()?;
    blob.record(&transaction, DIFF_MEDIA_TYPE, diff.read_at)?;
    transaction.execute(
      "INSERT INTO revision_diffs (base_sha, head_sha, diff_blob, read_at) VALUES (?1, ?2, ?3, ?4) \
       ON CONFLICT (base_sha, head_sha) DO UPDATE SET diff_blob = excluded.diff_blob, read_at = excluded.read_at",
      params![
        diff.base.as_str(),
        diff.head.as_str(),
        blob.hash.as_str(),
        diff.read_at.unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  fn revision_diff(&self, base: &Revision, head: &Revision) -> Result<Option<RevisionDiff>, StoreError> {
    let stored = self
      .connection
      .query_row(
        "SELECT base_sha, head_sha, diff_blob, read_at FROM revision_diffs WHERE base_sha = ?1 AND head_sha = ?2",
        params![base.as_str(), head.as_str()],
        StoredDiff::read,
      )
      .optional()?;
    stored.map(|stored| stored.load(self)).transpose()
  }
}
