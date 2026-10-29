//! `run_completions` and `run_artifacts`: runs completed without a merge (ORC-93), one row per run, and the texts a
//! run keeps by name, a completed run's report.

use orchester_core::{
  id::{RunId, TaskId},
  resource::StepKey,
  run::{CompletedBy, RunArtifact, RunCompletion, RunStatus},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// Records `completion` and keeps `report`, the run's report, as its artifact; sets the run's task Done, as a merge
  /// does, and keeps what the task took beside its points. Only a task's latest run that passed every step, was not
  /// merged and no merge waits in its worktree is completed, once.
  pub fn save_run_completion(
    &mut self,
    completion: &RunCompletion,
    report: Option<&RunArtifact>,
  ) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let run = completion.run;
    let status = super::runs::stored_status(&transaction, run)?.ok_or(StoreError::RunNotFound(run))?;
    if completed(&transaction, run)? {
      return Err(StoreError::AlreadyCompleted(run));
    }
    if !completable(&transaction, run, &status)? {
      return Err(StoreError::NotCompletable(run));
    }
    record(&transaction, completion, report)?;
    let task = task_of(&transaction, run)?;
    super::organization::automatic(
      &transaction,
      task,
      orchester_core::task::organization::WorkflowStatus::Done,
      completion.completed_at,
    )?;
    super::task_effort::record(&transaction, task, completion.completed_at)?;
    transaction.commit()?;
    Ok(())
  }

  /// Settles the completion `completion` names, of a run whose task was closed (Done or Canceled) while the run waited
  /// to be completed (ORC-150): records it and keeps `report` as save_run_completion does, and leaves the task's status
  /// as it was closed. Whether it settled it: a run completed already, or that save_run_completion would refuse, or
  /// whose task is not closed, is left as it is.
  pub fn settle_run_completion(
    &mut self,
    completion: &RunCompletion,
    report: Option<&RunArtifact>,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let run = completion.run;
    let Some(status) = super::runs::stored_status(&transaction, run)? else {
      return Ok(false);
    };
    if completed(&transaction, run)? || !completable(&transaction, run, &status)? {
      return Ok(false);
    }
    let task = task_of(&transaction, run)?;
    if !super::organization::load(&transaction, task)?.status().is_closed() {
      return Ok(false);
    }
    record(&transaction, completion, report)?;
    transaction.commit()?;
    Ok(true)
  }

  /// The latest run of `task` when it passed every step and was neither merged nor completed: one that may wait to be
  /// completed, as git tells whether it committed anything.
  pub fn uncompleted_latest_run(&self, task: TaskId) -> Result<Option<RunId>, StoreError> {
    let id: Option<String> = self
      .connection
      .query_row(
        "SELECT id FROM (SELECT id, status FROM runs WHERE task_id = ?1 ORDER BY number DESC LIMIT 1) \
         WHERE status = 'succeeded' AND id NOT IN (SELECT run_id FROM run_merges) \
         AND id NOT IN (SELECT run_id FROM run_completions)",
        [task.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    id.map(|id| parse("run id", &id)).transpose()
  }

  /// How `run` was completed without a merge, once it was.
  pub fn run_completion(&self, run: RunId) -> Result<Option<RunCompletion>, StoreError> {
    let row: Option<(String, i64)> = self
      .connection
      .query_row(
        "SELECT completed_by, completed_at FROM run_completions WHERE run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let Some((by, at)) = row else {
      return Ok(None);
    };
    Ok(Some(RunCompletion {
      run,
      by: by
        .parse::<CompletedBy>()
        .map_err(|error| corrupt("run completer", error))?,
      completed_at: Timestamp::from_unix_millis(at),
    }))
  }

  /// The texts `run` keeps, by name.
  pub fn run_artifacts(&self, run: RunId) -> Result<Vec<RunArtifact>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT name, step_key, text, created_at FROM run_artifacts WHERE run_id = ?1 ORDER BY name")?;
    let rows = statement
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, Option<String>>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(name, step, text, at)| {
        Ok(RunArtifact {
          run,
          name,
          step: step
            .map(|step| StepKey::new(&step).map_err(|error| corrupt("artifact step", error)))
            .transpose()?,
          text,
          created_at: Timestamp::from_unix_millis(at),
        })
      })
      .collect()
  }
}

/// Whether `run` was completed.
fn completed(transaction: &Transaction<'_>, run: RunId) -> Result<bool, StoreError> {
  Ok(transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM run_completions WHERE run_id = ?1)",
    [run.to_string()],
    |row| row.get(0),
  )?)
}

/// Whether `run`, whose stored status is `status`, may be completed: it is its task's latest run, passed every step,
/// was not merged and no merge waits in its worktree.
fn completable(transaction: &Transaction<'_>, run: RunId, status: &RunStatus) -> Result<bool, StoreError> {
  Ok(
    *status == RunStatus::Succeeded
      && super::resolutions::resolution(transaction, run)?.is_none()
      && !super::merges::merged(transaction, run)?
      && !later(transaction, run)?,
  )
}

/// Records `completion` and keeps `report`, when it is the run's.
fn record(
  transaction: &Transaction<'_>,
  completion: &RunCompletion,
  report: Option<&RunArtifact>,
) -> Result<(), StoreError> {
  let run = completion.run;
  transaction.execute(
    "INSERT INTO run_completions (run_id, completed_by, completed_at) VALUES (?1, ?2, ?3)",
    params![
      run.to_string(),
      completion.by.as_str(),
      completion.completed_at.unix_millis()
    ],
  )?;
  if let Some(report) = report.filter(|report| report.run == run) {
    save_artifact(transaction, report)?;
  }
  Ok(())
}

/// The task `run` belongs to.
fn task_of(transaction: &Transaction<'_>, run: RunId) -> Result<TaskId, StoreError> {
  let task: String = transaction.query_row("SELECT task_id FROM runs WHERE id = ?1", [run.to_string()], |row| {
    row.get(0)
  })?;
  parse("task id", &task)
}

/// Whether a later run of `run`'s task exists.
fn later(transaction: &Transaction<'_>, run: RunId) -> Result<bool, StoreError> {
  Ok(transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM runs r JOIN runs n ON n.task_id = r.task_id AND n.number > r.number \
     WHERE r.id = ?1)",
    [run.to_string()],
    |row| row.get(0),
  )?)
}

fn save_artifact(transaction: &Transaction<'_>, artifact: &RunArtifact) -> Result<(), StoreError> {
  transaction.execute(
    "INSERT INTO run_artifacts (run_id, name, step_key, text, created_at) VALUES (?1, ?2, ?3, ?4, ?5) \
     ON CONFLICT (run_id, name) DO UPDATE SET step_key = ?3, text = ?4, created_at = ?5",
    params![
      artifact.run.to_string(),
      artifact.name,
      artifact.step.as_ref().map(StepKey::as_str),
      artifact.text,
      artifact.created_at.unix_millis(),
    ],
  )?;
  Ok(())
}
