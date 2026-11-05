//! `run_merges`: runs whose work was merged into their base branch, one row per run.

use orchester_core::{
  id::{RunId, TaskId},
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::{BranchName, RunStatus},
  store::MergeStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl MergeStore for ProjectDb {
  type Error = StoreError;

  fn save_run_merge(&mut self, merge: &RunMerge) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    // A run merges as it succeeded: not once it was reopened and works again (a follow-up, a return of its work), nor
    // while a merge waits in its worktree for a person.
    let status = super::runs::stored_status(&transaction, merge.run)?.ok_or(StoreError::RunNotFound(merge.run))?;
    // A run that never succeeded may still be found merged outside Orchester; one reopened since is not merged.
    let reopened = status != RunStatus::Succeeded && succeeded_once(&transaction, merge.run)?;
    if reopened || super::resolutions::resolution(&transaction, merge.run)?.is_some() {
      return Err(StoreError::NotMergeable(merge.run));
    }
    let inserted = transaction.execute(
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
    let task: String = transaction.query_row(
      "SELECT task_id FROM runs WHERE id = ?1",
      [merge.run.to_string()],
      |row| row.get(0),
    )?;
    let task = crate::rows::parse("task id", &task)?;
    super::organization::automatic(
      &transaction,
      task,
      orchester_core::task::organization::WorkflowStatus::Done,
      merge.merged_at,
    )?;
    // Story points' calibration: what the task took, beside the points it had.
    super::task_effort::record(&transaction, task, merge.merged_at)?;
    transaction.commit()?;
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
    row.map(|row| merge_of(run, row)).transpose()
  }
}

impl ProjectDb {
  /// Runs Orchester merged, with their tasks, newest first: at most `limit` of those merged before `before` (when, and
  /// the run among those merged at the same moment), or the newest.
  pub fn merged_runs(
    &self,
    before: Option<(Timestamp, RunId)>,
    limit: usize,
  ) -> Result<Vec<(TaskId, RunMerge)>, StoreError> {
    // Without `before`, every merge is before the latest instant there is.
    let (at, run) = before.map_or((i64::MAX, String::new()), |(at, run)| {
      (at.unix_millis(), run.to_string())
    });
    let mut statement = self.connection.prepare(
      "SELECT m.run_id, r.task_id, m.into_branch, m.revision, m.kind, m.merged_at
       FROM run_merges m JOIN runs r ON r.id = m.run_id
       WHERE m.merged_at < ?2 OR (m.merged_at = ?2 AND m.run_id < ?1)
       ORDER BY m.merged_at DESC, m.run_id DESC
       LIMIT ?3",
    )?;
    // A limit past SQLite's integers is no limit.
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let rows = statement.query_map(params![run, at, limit], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        (row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?),
      ))
    })?;
    rows
      .map(|row| {
        let (run, task, merge) = row?;
        Ok((
          crate::rows::parse("task id", &task)?,
          merge_of(crate::rows::parse("run id", &run)?, merge)?,
        ))
      })
      .collect()
  }
}

fn merge_of(
  run: RunId,
  (into, revision, kind, merged_at): (String, String, String, i64),
) -> Result<RunMerge, StoreError> {
  Ok(RunMerge {
    run,
    into: BranchName::new(&into).map_err(|error| corrupt("merged branch", error))?,
    revision: Revision::new(&revision).map_err(|error| corrupt("merge revision", error))?,
    kind: parse_kind(&kind)?,
    merged_at: Timestamp::from_unix_millis(merged_at),
  })
}

const fn kind_text(kind: MergeKind) -> &'static str {
  match kind {
    MergeKind::FastForward => "fast_forward",
    MergeKind::MergeCommit => "merge_commit",
    MergeKind::AlreadyMerged => "already_merged",
    MergeKind::Squash => "squash",
  }
}

fn parse_kind(text: &str) -> Result<MergeKind, StoreError> {
  match text {
    "fast_forward" => Ok(MergeKind::FastForward),
    "merge_commit" => Ok(MergeKind::MergeCommit),
    "already_merged" => Ok(MergeKind::AlreadyMerged),
    "squash" => Ok(MergeKind::Squash),
    other => Err(corrupt("merge kind", other)),
  }
}

/// Whether `run`'s work was merged.
pub(super) fn merged(connection: &rusqlite::Connection, run: RunId) -> Result<bool, StoreError> {
  Ok(connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM run_merges WHERE run_id = ?1)",
    [run.to_string()],
    |row| row.get(0),
  )?)
}

/// Whether an attempt of `run` succeeded: the run ended so once, whatever it does now.
fn succeeded_once(connection: &rusqlite::Connection, run: RunId) -> Result<bool, StoreError> {
  Ok(connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM attempts WHERE run_id = ?1 AND status = 'succeeded')",
    [run.to_string()],
    |row| row.get(0),
  )?)
}
