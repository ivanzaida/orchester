//! `run_merges`: runs whose work was merged into their base branch, one row per run.

use std::collections::HashMap;

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
    // Story points' calibration: what the task took, beside the points it had. Read before the write transaction, which
    // holds the runner's writes off until it commits: the transcripts it reads are long (ORC-914).
    let (task, effort) = super::task_effort::of_run(&self.connection, merge.run)?;
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
    // A run of inputs the team server accepted never changes its task here: the server keeps the task's status, and
    // whether a merged team run makes its task Done is open (D4). A Done its claim's end recorded first, from git, and
    // did not send stays not sent (ORC-750).
    if !super::run_inputs::is_team_run(&transaction, merge.run)?
      && !super::automatic_status::merged_done_not_sent(&transaction, task, merge.run)?
    {
      super::organization::automatic(
        &transaction,
        task,
        orchester_core::task::organization::WorkflowStatus::Done,
        merge.merged_at,
      )?;
    }
    super::task_effort::record(&transaction, task, &effort, merge.merged_at)?;
    transaction.commit()?;
    Ok(())
  }

  fn run_merge(&self, run: RunId) -> Result<Option<RunMerge>, StoreError> {
    let row: Option<MergeColumns> = self
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

  /// [`MergeStore::run_merge`] of every merged run, in one statement.
  pub fn run_merges(&self) -> Result<HashMap<RunId, RunMerge>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT run_id, into_branch, revision, kind, merged_at FROM run_merges")?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          (row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?),
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(run, columns)| {
        let run = crate::rows::parse("run id", &run)?;
        Ok((run, merge_of(run, columns)?))
      })
      .collect()
  }
}

/// `into_branch`, `revision`, `kind` and `merged_at` of a merge.
type MergeColumns = (String, String, String, i64);

/// The merge of `run` these columns record.
fn merge_of(run: RunId, (into, revision, kind, merged_at): MergeColumns) -> Result<RunMerge, StoreError> {
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
