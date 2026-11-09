mod archived;
mod children;
mod columns;
mod create;
mod legacy;
mod load;
mod record;

use std::{collections::BTreeMap, num::NonZeroU32};

use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId, TaskId},
  resource::VersionNumber,
  run::{AttemptReason, BranchName, Checkpoint, ListedRun, NewRun, Plan, Run, RunEvent, RunEventKind, RunNumber},
  store::RunStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub(super) use self::load::sequence_value;
pub use self::record::RunLogRecord;
use self::{
  columns::run_status_columns,
  load::{load_checkpoint, load_events, load_runs},
};
use super::{ProjectDb, plans, resources::version_number, write_transaction::retried};
use crate::{
  RunStoreError, StoreError,
  rows::{corrupt, encode_json, optional_budget_column, optional_millis_column, tokens_column},
};

const SNAPSHOT_SCHEMA: u32 = 1;
const PERMISSIONS_SCHEMA: u32 = 1;
/// Schema 2: token budgets. Schema 1 logs are upgraded when read (`legacy`).
const EVENT_SCHEMA: u32 = 2;

/// A run's task and the branch it works on, read without its record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunBranch {
  pub run: RunId,
  pub task: TaskId,
  pub number: RunNumber,
  pub branch: BranchName,
}

impl ProjectDb {
  /// Every run's task and branch, by number: what names a pull request's head branch. Reads no run's record,
  /// so a run whose record cannot be read is listed too. A run that never started (`failure_reason` says why) made
  /// no branch, so it is left out ([`orchester_core::run::Run::own_branch`]).
  pub fn run_branches(&self) -> Result<Vec<RunBranch>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT id, task_id, number, branch FROM runs WHERE failure_reason IS NULL ORDER BY number")?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, i64>(2)?,
          row.get::<_, String>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(run, task, number, branch)| {
        Ok(RunBranch {
          run: crate::rows::parse("run id", &run)?,
          task: crate::rows::parse("task id", &task)?,
          number: run_number(number)?,
          branch: BranchName::new(&branch).map_err(|error| corrupt("run branch", error))?,
        })
      })
      .collect()
  }

  /// Runs stopped at their account's plan limit, ordered by number; each waits for the limit's reset or a person.
  /// Runs that have not ended (succeeded, failed or cancelled), by number: each may still start an agent.
  pub fn unfinished_runs(&self) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE status NOT IN ('succeeded', 'failed', 'cancelled') ORDER BY number",
      &[],
    )?)
  }

  /// Succeeded runs that recorded no checkpoint, by number: runners before ORC-147 recorded none when the agents
  /// committed their work themselves.
  pub fn succeeded_runs_without_checkpoint(&self) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE status = 'succeeded' AND id NOT IN (SELECT run_id FROM checkpoints) ORDER BY number",
      &[],
    )?)
  }

  pub fn runs_at_plan_limit(&self) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE status = 'paused' AND status_reason = 'plan_limit' ORDER BY number",
      &[],
    )?)
  }
}

impl RunStore for ProjectDb {
  type Error = RunStoreError;

  /// Records the run without checking the task's status, which tests use to set up runs of
  /// drafts. Production code records runs with [`ProjectDb::create_run_of_queued_task`], which
  /// rechecks that the task is still queued.
  fn create_run(&mut self, key: IdempotencyKey, new: NewRun, now: Timestamp) -> Result<Run, RunStoreError> {
    self.record_run(key, new, now, None, None)
  }

  fn run_with_key(&self, key: IdempotencyKey) -> Result<Option<Run>, RunStoreError> {
    load_runs(&self.connection, "WHERE idempotency_key = ?1", params![key.to_string()])?
      .pop()
      .transpose()
      .map_err(|unreadable| unreadable.error)
  }

  fn next_run_number(&self) -> Result<RunNumber, RunStoreError> {
    let number: i64 = self
      .connection
      .query_row("SELECT next FROM counters WHERE name = 'run'", [], |row| row.get(0))?;
    Ok(run_number(number)?)
  }

  fn append_events(&mut self, run: &Run, events: &[RunEvent]) -> Result<(), RunStoreError> {
    self.write_run_log(&RunLogRecord::events(run, events)?)
  }

  fn append_events_with_plan(&mut self, run: &Run, events: &[RunEvent], plan: &Plan) -> Result<(), RunStoreError> {
    self.write_run_log(&RunLogRecord::with_plan(run, events, plan)?)
  }

  fn plans(&self, run: RunId) -> Result<Vec<Plan>, RunStoreError> {
    Ok(plans::of_run(&self.connection, run)?)
  }

  fn run(&self, id: RunId) -> Result<Option<Run>, RunStoreError> {
    load_runs(&self.connection, "WHERE id = ?1", params![id.to_string()])?
      .pop()
      .transpose()
      .map_err(|unreadable| unreadable.error)
  }

  fn runs_of_task(&self, task: TaskId) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE task_id = ?1 ORDER BY number",
      params![task.to_string()],
    )?)
  }

  fn events(&self, run: RunId) -> Result<Vec<RunEvent>, RunStoreError> {
    Ok(load_events(&self.connection, run)?)
  }

  fn latest_checkpoint(&self, run: RunId) -> Result<Option<Checkpoint>, RunStoreError> {
    Ok(load_checkpoint(&self.connection, run)?)
  }

  fn interrupted_runs(&self) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE status = ?1 AND id NOT IN (SELECT run_id FROM run_worktree_removals) ORDER BY number",
      params![columns::INTERRUPTED],
    )?)
  }

  fn running_runs(&self) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE status = ?1 ORDER BY number",
      params![columns::RUNNING],
    )?)
  }

  fn low_disk_runs(&self) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE status = 'paused' AND status_reason = ?1 ORDER BY number",
      params!["low_disk"],
    )?)
  }

  fn runs_by_pipeline_version(
    &self,
    pipeline: ResourceId,
  ) -> Result<BTreeMap<VersionNumber, Vec<RunNumber>>, RunStoreError> {
    let mut statement = self.connection.prepare(
      "SELECT resource_versions.number, runs.number FROM runs \
       JOIN resource_versions ON resource_versions.id = runs.pipeline_version_id \
       WHERE resource_versions.resource_id = ?1 ORDER BY runs.number",
    )?;
    let rows = statement
      .query_map([pipeline.to_string()], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let mut runs: BTreeMap<VersionNumber, Vec<RunNumber>> = BTreeMap::new();
    for (version, run) in rows {
      runs.entry(version_number(version)?).or_default().push(run_number(run)?);
    }
    Ok(runs)
  }
}

impl ProjectDb {
  /// Writes `record`: appends its events and stores the change they make to the run and what is kept with them, in
  /// one transaction, run again while the database is busy ([`retried`]). The first event must directly follow the
  /// stored log; otherwise another writer appended first and the call fails with a sequence conflict, changing
  /// nothing. The runner's records come here through the coordinator's sink.
  pub fn write_run_log(&mut self, record: &RunLogRecord) -> Result<(), RunStoreError> {
    record.check()?;
    retried(&mut self.connection, "append run events", |transaction| {
      append_in(transaction, record)
    })
  }
}

/// [`ProjectDb::write_run_log`] in `transaction`.
fn append_in(transaction: &Transaction<'_>, record: &RunLogRecord) -> Result<(), RunStoreError> {
  let (run, task, events) = (record.run(), record.task(), record.appended());
  let continues = events.iter().any(|event| event.kind().starts_execution());
  // Transactions take the write lock before they read (`migrate`), so a continuation and a removal
  // of the run's worktrees (`WorktreeStore::begin_worktree_removal`) each see the other.
  if continues && super::worktree_removals::removal(transaction, run)?.is_some() {
    return Err(RunStoreError::WorktreeRemoved(run));
  }
  if continues && super::resolutions::resolution(transaction, run)?.is_some() {
    return Err(RunStoreError::Resolving(run));
  }
  // Work returned after the run ended goes on only while it is not merged: the merge stands for what the run did.
  if events.iter().any(returns_after_end) && super::merges::merged(transaction, run)? {
    return Err(RunStoreError::Merged(run));
  }
  let stored: Option<i64> = transaction
    .query_row(
      "SELECT last_sequence FROM runs WHERE id = ?1",
      [run.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  let stored = stored.ok_or(RunStoreError::RunNotFound(run))?;
  let expected = u64::try_from(stored).map_err(|error| corrupt("run sequence", error))? + 1;
  let found = events[0].sequence().get();
  if found != expected {
    return Err(RunStoreError::SequenceConflict { run, expected, found });
  }
  // A team task whose definition is kept only on its server has no readiness or workflow status here, and a run of
  // inputs the team server accepted never changes its task here, even one with a cached definition: its run goes on
  // from its inputs, and the server keeps the task's status.
  let defined =
    super::task_identities::has_definition(transaction, task)? && !super::run_inputs::is_team_run(transaction, run)?;
  if continues && defined {
    super::organization::check_readiness(transaction, task)?;
  }
  record.store_change(transaction)?;
  for event in events {
    insert_event(transaction, event)?;
    if defined {
      follow_workflow(transaction, task, event)?;
    }
  }
  record.store_kept(transaction)?;
  Ok(())
}

/// Moves `task` to the workflow status `event` brings it to, if any: In progress when an agent starts, In review when
/// the run succeeds.
fn follow_workflow(transaction: &Transaction<'_>, task: TaskId, event: &RunEvent) -> Result<(), StoreError> {
  use orchester_core::task::organization::WorkflowStatus;
  let status = match event.kind() {
    kind if kind.starts_execution() => WorkflowStatus::InProgress,
    RunEventKind::Succeeded | RunEventKind::FollowUpFailed => WorkflowStatus::InReview,
    _ => return Ok(()),
  };
  super::organization::automatic(transaction, task, status, event.at())
}

fn insert_run(transaction: &Transaction<'_>, run: &Run, key: &str) -> Result<(), StoreError> {
  let columns = run_status_columns(run.status());
  let (base_source, base_module) = super::base_branch::source_columns(run.base_source());
  transaction.execute(
    "INSERT INTO runs (id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch, base_revision, \
     initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested, \
     status, status_reason, failed_step, limit_execution, last_sequence, created_at, base_branch, failure_reason, \
     base_source, base_module) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, \
     ?23, ?24)",
    params![
      run.id().to_string(),
      run.number().get(),
      key,
      run.task().to_string(),
      run.snapshot().pipeline().version.to_string(),
      encode_json("run snapshot", SNAPSHOT_SCHEMA, run.snapshot())?,
      run.branch().as_str(),
      run.base().as_str(),
      optional_budget_column("run budget", run.created().budget)?,
      optional_millis_column("run time limit", run.created().max_duration)?,
      optional_budget_column("run budget", run.budget())?,
      optional_millis_column("run time limit", run.max_duration())?,
      tokens_column("run usage", run.used().get())?,
      run.pause_requested(),
      columns.status,
      columns.reason,
      columns.failed_step,
      columns.limit_execution,
      sequence_column(run.last_sequence().get())?,
      run.created_at().unix_millis(),
      run.base_branch().map(BranchName::as_str),
      columns.failure_reason,
      base_source,
      base_module,
    ],
  )?;
  Ok(())
}

/// Appends one event; a recorded checkpoint also gets its immutable row.
fn insert_event(transaction: &Transaction<'_>, event: &RunEvent) -> Result<(), StoreError> {
  let run = event.run().to_string();
  let sequence = sequence_column(event.sequence().get())?;
  let at = event.at().unix_millis();
  transaction.execute(
    "INSERT INTO run_events (run_id, sequence, at, kind, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
    params![
      run,
      sequence,
      at,
      event.kind().name(),
      encode_json("run event", EVENT_SCHEMA, event.kind())?
    ],
  )?;
  if let RunEventKind::CheckpointRecorded {
    checkpoint,
    execution,
    revision,
  } = event.kind()
  {
    transaction.execute(
      "INSERT INTO checkpoints (id, run_id, step_execution_id, revision, sequence, created_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![
        checkpoint.to_string(),
        run,
        execution.to_string(),
        revision.as_str(),
        sequence,
        at
      ],
    )?;
  }
  Ok(())
}

fn sequence_column(sequence: u64) -> Result<i64, StoreError> {
  i64::try_from(sequence).map_err(|_| StoreError::Unrepresentable("run event sequence"))
}

fn run_number(number: i64) -> Result<RunNumber, StoreError> {
  u32::try_from(number)
    .ok()
    .and_then(NonZeroU32::new)
    .map(RunNumber::new)
    .ok_or_else(|| corrupt("run number", number))
}

/// `status`, `status_reason`, `failed_step`, `limit_execution` and `failure_reason` as stored.
type StatusRow = (String, Option<String>, Option<String>, Option<String>, Option<String>);

/// The status of run `run` as `connection` reads it; `None` when there is no such run.
pub(super) fn stored_status(
  connection: &Connection,
  run: RunId,
) -> Result<Option<orchester_core::run::RunStatus>, StoreError> {
  let columns: Option<StatusRow> = connection
    .query_row(
      "SELECT status, status_reason, failed_step, limit_execution, failure_reason FROM runs WHERE id = ?1",
      [run.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )
    .optional()?;
  columns
    .map(|(status, reason, failed_step, limit, failure)| {
      columns::parse_run_status(
        &status,
        reason.as_deref(),
        (failed_step.as_deref(), limit.as_deref()),
        failure.as_deref(),
      )
    })
    .transpose()
}

/// Whether `event` starts an attempt that returns the work of an ended run.
fn returns_after_end(event: &RunEvent) -> bool {
  matches!(
    event.kind(),
    RunEventKind::AttemptStarted {
      reason: AttemptReason::ReturnedAfterEnd { .. },
      ..
    }
  )
}
