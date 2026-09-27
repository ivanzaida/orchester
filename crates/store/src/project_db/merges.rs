//! `run_merges`: runs whose work was merged into their base branch, one row per run.

use orchester_core::{
  id::RunId,
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::MergeStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl MergeStore for ProjectDb {
  type Error = StoreError;

  fn save_run_merge(&mut self, merge: &RunMerge) -> Result<(), StoreError> {
    let inserted = self.connection.execute(
      "INSERT INTO run_merges (run_id, into_branch, revision, kind, merged_at) VALUES (?1, ?2, ?3, ?4, ?5) \
       ON CONFLICT (run_id) DO NOTHING",
      params![
        merge.run.to_string(),
        merge.into.as_str(),
        merge.revision.as_str(),
        kind_text(merge.kind),
        merge.merged_at.unix_millis(),
      ],
    )?;
    if inserted == 0 {
      return Err(StoreError::AlreadyMerged(merge.run));
    }
    Ok(())
  }

  fn run_merge(&self, run: RunId) -> Result<Option<RunMerge>, StoreError> {
    let row: Option<(String, String, String, i64)> = self
      .connection
      .query_row(
        "SELECT into_branch, revision, kind, merged_at FROM run_merges WHERE run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?;
    let Some((into, revision, kind, merged_at)) = row else {
      return Ok(None);
    };
    Ok(Some(RunMerge {
      run,
      into: BranchName::new(&into).map_err(|error| corrupt("merged branch", error))?,
      revision: Revision::new(&revision).map_err(|error| corrupt("merge revision", error))?,
      kind: parse_kind(&kind)?,
      merged_at: Timestamp::from_unix_millis(merged_at),
    }))
  }
}

const fn kind_text(kind: MergeKind) -> &'static str {
  match kind {
    MergeKind::FastForward => "fast_forward",
    MergeKind::MergeCommit => "merge_commit",
    MergeKind::AlreadyMerged => "already_merged",
  }
}

fn parse_kind(text: &str) -> Result<MergeKind, StoreError> {
  match text {
    "fast_forward" => Ok(MergeKind::FastForward),
    "merge_commit" => Ok(MergeKind::MergeCommit),
    "already_merged" => Ok(MergeKind::AlreadyMerged),
    other => Err(corrupt("merge kind", other)),
  }
}
