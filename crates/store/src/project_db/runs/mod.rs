mod columns;
mod legacy;
mod load;

use std::{collections::BTreeMap, num::NonZeroU32};

use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId, TaskId},
  resource::VersionNumber,
  run::{BranchName, Checkpoint, ListedRun, NewRun, Plan, Run, RunEvent, RunEventKind, RunNumber, StepExecutor},
  store::RunStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, params};

use self::{
  columns::{attempt_reason_columns, attempt_status_text, run_status_columns, step_failure_text, step_status_columns},
  load::{load_checkpoint, load_events, load_runs},
};
use super::{ProjectDb, plans, resources::version_number};
use crate::{
  RunStoreError, StoreError,
  rows::{corrupt, encode_json, millis_column, tokens_column},
};

const SNAPSHOT_SCHEMA: u32 = 1;
const PERMISSIONS_SCHEMA: u32 = 1;
/// Schema 2: token budgets. Schema 1 logs are upgraded when read (`legacy`).
const EVENT_SCHEMA: u32 = 2;

impl RunStore for ProjectDb {
  type Error = RunStoreError;

  fn create_run(&mut self, key: IdempotencyKey, new: NewRun, now: Timestamp) -> Result<Run, RunStoreError> {
    let key = key.to_string();
    let transaction = self.connection.transaction()?;
    if let Some(existing) = load_runs(&transaction, "WHERE idempotency_key = ?1", [&key])?.pop() {
      return existing.map_err(|unreadable| unreadable.error);
    }
    let task_exists: bool = transaction.query_row(
      "SELECT EXISTS (SELECT 1 FROM tasks WHERE id = ?1)",
      [new.task.to_string()],
      |row| row.get(0),
    )?;
    if !task_exists {
      return Err(StoreError::TaskNotFound(new.task).into());
    }
    let number: i64 = transaction.query_row("SELECT next FROM counters WHERE name = 'run'", [], |row| row.get(0))?;
    let (run, created) = Run::create(RunId::generate(), run_number(number)?, new, now)?;
    insert_run(&transaction, &run, &key)?;
    insert_event(&transaction, &created)?;
    transaction.execute("UPDATE counters SET next = ?1 WHERE name = 'run'", [number + 1])?;
    transaction.commit()?;
    Ok(run)
  }

  fn append_events(&mut self, run: &Run, events: &[RunEvent]) -> Result<(), RunStoreError> {
    self.append(run, events, None)
  }

  fn append_events_with_plan(&mut self, run: &Run, events: &[RunEvent], plan: &Plan) -> Result<(), RunStoreError> {
    if plan.run != run.id() {
      return Err(RunStoreError::EventsMismatch("the plan belongs to another run"));
    }
    self.append(run, events, Some(plan))
  }

  fn plans(&self, run: RunId) -> Result<Vec<Plan>, RunStoreError> {
    Ok(plans::of_run(&self.connection, run)?)
  }

  fn run(&self, id: RunId) -> Result<Option<Run>, RunStoreError> {
    load_runs(&self.connection, "WHERE id = ?1", [id.to_string()])?
      .pop()
      .transpose()
      .map_err(|unreadable| unreadable.error)
  }

  fn runs_of_task(&self, task: TaskId) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE task_id = ?1 ORDER BY number",
      [task.to_string()],
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
      "WHERE status = ?1 ORDER BY number",
      [columns::INTERRUPTED],
    )?)
  }

  fn running_runs(&self) -> Result<Vec<ListedRun<RunStoreError>>, RunStoreError> {
    Ok(load_runs(
      &self.connection,
      "WHERE status = ?1 ORDER BY number",
      [columns::RUNNING],
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
  /// Appends `events` and stores `run`, and `plan` if given, in one transaction.
  fn append(&mut self, run: &Run, events: &[RunEvent], plan: Option<&Plan>) -> Result<(), RunStoreError> {
    check_events(run, events)?;
    let transaction = self.connection.transaction()?;
    let stored: Option<i64> = transaction
      .query_row(
        "SELECT last_sequence FROM runs WHERE id = ?1",
        [run.id().to_string()],
        |row| row.get(0),
      )
      .optional()?;
    let stored = stored.ok_or(RunStoreError::RunNotFound(run.id()))?;
    let expected = u64::try_from(stored).map_err(|error| corrupt("run sequence", error))? + 1;
    let found = events[0].sequence().get();
    if found != expected {
      return Err(RunStoreError::SequenceConflict {
        run: run.id(),
        expected,
        found,
      });
    }
    update_run(&transaction, run)?;
    write_attempts(&transaction, run)?;
    for event in events {
      insert_event(&transaction, event)?;
    }
    if let Some(plan) = plan {
      plans::save(&transaction, plan)?;
    }
    transaction.commit()?;
    Ok(())
  }
}

/// `events` must be the consecutive events of `run` that end in its current state.
fn check_events(run: &Run, events: &[RunEvent]) -> Result<(), RunStoreError> {
  let last = events
    .last()
    .ok_or(RunStoreError::EventsMismatch("there are no events to append"))?;
  if events.iter().any(|event| event.run() != run.id()) {
    return Err(RunStoreError::EventsMismatch("an event belongs to another run"));
  }
  let consecutive = events
    .windows(2)
    .all(|pair| pair[0].sequence().next() == pair[1].sequence());
  if !consecutive {
    return Err(RunStoreError::EventsMismatch("the events are not consecutive"));
  }
  if last.sequence() != run.last_sequence() {
    return Err(RunStoreError::EventsMismatch(
      "the last event does not produce the stored state",
    ));
  }
  Ok(())
}

fn insert_run(transaction: &Transaction<'_>, run: &Run, key: &str) -> Result<(), StoreError> {
  let columns = run_status_columns(run.status());
  transaction.execute(
    "INSERT INTO runs (id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch, base_revision, \
     initial_budget_tokens, initial_max_duration_ms, budget_tokens, max_duration_ms, used_tokens, pause_requested, \
     status, status_reason, failed_step, limit_execution, last_sequence, created_at, base_branch) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
    params![
      run.id().to_string(),
      run.number().get(),
      key,
      run.task().to_string(),
      run.snapshot().pipeline().version.to_string(),
      encode_json("run snapshot", SNAPSHOT_SCHEMA, run.snapshot())?,
      run.branch().as_str(),
      run.base().as_str(),
      tokens_column("run budget", run.created().budget.get())?,
      millis_column("run time limit", run.created().max_duration)?,
      tokens_column("run budget", run.budget().get())?,
      millis_column("run time limit", run.max_duration())?,
      tokens_column("run usage", run.used().get())?,
      run.pause_requested(),
      columns.status,
      columns.reason,
      columns.failed_step,
      columns.limit_execution,
      sequence_column(run.last_sequence().get())?,
      run.created_at().unix_millis(),
      run.base_branch().map(BranchName::as_str),
    ],
  )?;
  Ok(())
}

fn update_run(transaction: &Transaction<'_>, run: &Run) -> Result<(), StoreError> {
  let columns = run_status_columns(run.status());
  transaction.execute(
    "UPDATE runs SET status = ?2, status_reason = ?3, failed_step = ?4, limit_execution = ?5, budget_tokens = ?6, \
     max_duration_ms = ?7, used_tokens = ?8, pause_requested = ?9, last_sequence = ?10 WHERE id = ?1",
    params![
      run.id().to_string(),
      columns.status,
      columns.reason,
      columns.failed_step,
      columns.limit_execution,
      tokens_column("run budget", run.budget().get())?,
      millis_column("run time limit", run.max_duration())?,
      tokens_column("run usage", run.used().get())?,
      run.pause_requested(),
      sequence_column(run.last_sequence().get())?,
    ],
  )?;
  let mut step_budget = transaction.prepare_cached(
    "INSERT INTO run_step_budgets (run_id, step_key, budget_tokens) VALUES (?1, ?2, ?3) \
     ON CONFLICT (run_id, step_key) DO UPDATE SET budget_tokens = excluded.budget_tokens",
  )?;
  for (step, budget) in run.raised_step_budgets() {
    step_budget.execute(params![
      run.id().to_string(),
      step.as_str(),
      tokens_column("step budget", budget.get())?
    ])?;
  }
  let mut permissions = transaction.prepare_cached(
    "INSERT INTO run_permissions (run_id, profile_id, body) VALUES (?1, ?2, ?3)      ON CONFLICT (run_id, profile_id) DO UPDATE SET body = excluded.body",
  )?;
  for (profile, changed) in run.all_changed_permissions() {
    permissions.execute(params![
      run.id().to_string(),
      profile.to_string(),
      encode_json("run permissions", PERMISSIONS_SCHEMA, changed)?
    ])?;
  }
  Ok(())
}

/// Inserts new attempts and executions and updates the status of existing ones.
fn write_attempts(transaction: &Transaction<'_>, run: &Run) -> Result<(), StoreError> {
  let mut attempt_statement = transaction.prepare_cached(
    "INSERT INTO attempts (id, run_id, number, reason, failed_step, return_to, status) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (id) DO UPDATE SET status = excluded.status",
  )?;
  let mut execution_statement = transaction.prepare_cached(
    "INSERT INTO step_executions (id, attempt_id, step_key, profile_version_id, model, status, wait_reason, failure) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
     ON CONFLICT (id) DO UPDATE SET status = excluded.status, wait_reason = excluded.wait_reason, \
     failure = excluded.failure",
  )?;
  for attempt in run.attempts() {
    let (reason, failed_step, return_to) = attempt_reason_columns(attempt.reason());
    attempt_statement.execute(params![
      attempt.id().to_string(),
      run.id().to_string(),
      attempt.number().get(),
      reason,
      failed_step,
      return_to,
      attempt_status_text(attempt.status()),
    ])?;
    for execution in attempt.executions() {
      let (profile_version, model) = match run.snapshot().step(execution.step()).map(|step| step.executor()) {
        Some(StepExecutor::Agent { profile, model, .. }) => (Some(profile.version.to_string()), Some(model.as_str())),
        _ => (None, None),
      };
      let (status, wait_reason) = step_status_columns(execution.status());
      execution_statement.execute(params![
        execution.id().to_string(),
        attempt.id().to_string(),
        execution.step().as_str(),
        profile_version,
        model,
        status,
        wait_reason,
        execution.failure().map(step_failure_text),
      ])?;
    }
  }
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
