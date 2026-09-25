//! `pr_revisions`: the diff of a pull request per head revision, its text in a blob.

use orchester_core::{
  pull_request::RevisionDiff, repository::GitHubRepo, revision::Revision, store::PullRequestDiffStore, time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::{
  ProjectDb,
  blobs::{self, BlobHash},
};
use crate::{StoreError, rows::corrupt};

/// How a diff blob is labelled in `blobs`.
const DIFF_MEDIA_TYPE: &str = "text/x-diff";
const COLUMNS: &str = "base_sha, head_sha, diff_blob, synced_at";

impl PullRequestDiffStore for ProjectDb {
  type Error = StoreError;

  fn save_pull_request_diff(&mut self, repo: &GitHubRepo, number: u64, diff: &RevisionDiff) -> Result<(), StoreError> {
    let number = stored_number(number)?;
    let transaction = self.connection.transaction()?;
    let blob = blobs::put(
      &transaction,
      &self.directory,
      DIFF_MEDIA_TYPE,
      diff.text.as_bytes(),
      diff.synced_at,
    )?;
    transaction.execute(
      "INSERT INTO pr_revisions (host, owner, name, number, head_sha, base_sha, diff_blob, synced_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
       ON CONFLICT (host, owner, name, number, head_sha) \
       DO UPDATE SET base_sha = excluded.base_sha, diff_blob = excluded.diff_blob, synced_at = excluded.synced_at",
      params![
        repo.host().as_str(),
        repo.owner(),
        repo.name(),
        number,
        diff.head.as_str(),
        diff.base.as_str(),
        blob.as_str(),
        diff.synced_at.unix_millis(),
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  fn pull_request_diff(
    &self,
    repo: &GitHubRepo,
    number: u64,
    head: &Revision,
  ) -> Result<Option<RevisionDiff>, StoreError> {
    let stored = self
      .connection
      .query_row(
        &format!(
          "SELECT {COLUMNS} FROM pr_revisions \
           WHERE host = ?1 AND owner = ?2 AND name = ?3 AND number = ?4 AND head_sha = ?5"
        ),
        params![
          repo.host().as_str(),
          repo.owner(),
          repo.name(),
          stored_number(number)?,
          head.as_str()
        ],
        StoredDiff::read,
      )
      .optional()?;
    stored.map(|stored| stored.load(self)).transpose()
  }

  fn latest_pull_request_diff(&self, repo: &GitHubRepo, number: u64) -> Result<Option<RevisionDiff>, StoreError> {
    let stored = self
      .connection
      .query_row(
        &format!(
          "SELECT {COLUMNS} FROM pr_revisions WHERE host = ?1 AND owner = ?2 AND name = ?3 AND number = ?4 \
           ORDER BY synced_at DESC LIMIT 1"
        ),
        params![repo.host().as_str(), repo.owner(), repo.name(), stored_number(number)?],
        StoredDiff::read,
      )
      .optional()?;
    stored.map(|stored| stored.load(self)).transpose()
  }
}

fn stored_number(number: u64) -> Result<i64, StoreError> {
  i64::try_from(number).map_err(|_| StoreError::Unrepresentable("pull request number"))
}

struct StoredDiff {
  base: String,
  head: String,
  blob: String,
  synced_at: i64,
}

impl StoredDiff {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      base: row.get(0)?,
      head: row.get(1)?,
      blob: row.get(2)?,
      synced_at: row.get(3)?,
    })
  }

  fn load(self, db: &ProjectDb) -> Result<RevisionDiff, StoreError> {
    let revision = |value: &str| Revision::new(value).map_err(|error| corrupt("pull request revision", error));
    let content = blobs::get(&db.connection, &db.directory, &BlobHash::stored(self.blob)?)?;
    Ok(RevisionDiff {
      base: revision(&self.base)?,
      head: revision(&self.head)?,
      text: String::from_utf8(content).map_err(|error| corrupt("pull request diff", error))?,
      synced_at: Timestamp::from_unix_millis(self.synced_at),
    })
  }
}
