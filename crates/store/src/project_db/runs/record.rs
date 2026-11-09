//! What the runner records in a run's log, as values that can be sent (`docs/architecture/remote-runners.md`, rule
//! 2): events, the change they make to the run's rows and what is kept with them. The change is the one
//! [`ProjectDb::write_run_log`] applies, column by column, not the run's whole record: `runs` updated in place, the
//! `run_step_budgets` and `run_permissions` it upserts, and every attempt and step execution upserted with the
//! `ON CONFLICT` updates the tables allow.

use orchester_core::{
  id::{RunId, TaskId},
  run::{EventSequence, InterruptCause, Plan, Run, RunEvent, RunEventKind, RunnerFailure},
};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};

use super::{
  PERMISSIONS_SCHEMA,
  columns::{
    attempt_conflicts_column, attempt_profile_column, attempt_reason_columns, attempt_status_text, run_status_columns,
    step_failure_text, step_status_columns,
  },
  sequence_column,
};
use crate::{
  RunStoreError, StoreError,
  rows::{encode_json, optional_budget_column, optional_millis_column, tokens_column},
};

/// Events of one run, the change they make to its rows, and what is kept with them: written in one transaction, or
/// not at all. The events must directly follow the stored log and end in the state the change stores.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunLogRecord {
  change: RunChange,
  events: Vec<RunEvent>,
  with: KeptWith,
}

/// What is kept with a record's events, in their transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum KeptWith {
  Nothing,
  /// The plan the events propose, or the decision on a stored one.
  Plan {
    plan: Box<Plan>,
  },
  /// Why the runner failed, kept with its interruption: the last of the events.
  RunnerFailure {
    failure: RunnerFailure,
  },
}

impl RunLogRecord {
  /// `events`, which produced `run`'s current state ([`orchester_core::store::RunStore::append_events`]).
  pub fn events(run: &Run, events: &[RunEvent]) -> Result<Self, RunStoreError> {
    Self::new(run, events, KeptWith::Nothing)
  }

  /// `events` with `plan`, the plan they propose or the decision on a stored one
  /// ([`orchester_core::store::RunStore::append_events_with_plan`]).
  pub fn with_plan(run: &Run, events: &[RunEvent], plan: &Plan) -> Result<Self, RunStoreError> {
    Self::new(
      run,
      events,
      KeptWith::Plan {
        plan: Box::new(plan.clone()),
      },
    )
  }

  /// `events` that end in the runner's interruption of the run after its own failure, with why
  /// ([`ProjectDb::append_runner_failure`](crate::ProjectDb::append_runner_failure)).
  pub fn runner_failure(run: &Run, events: &[RunEvent], failure: RunnerFailure) -> Result<Self, RunStoreError> {
    Self::new(run, events, KeptWith::RunnerFailure { failure })
  }

  fn new(run: &Run, events: &[RunEvent], with: KeptWith) -> Result<Self, RunStoreError> {
    Ok(Self {
      change: RunChange::of(run)?,
      events: events.to_vec(),
      with,
    })
  }

  pub(super) const fn run(&self) -> RunId {
    self.change.run
  }

  pub(super) const fn task(&self) -> TaskId {
    self.change.task
  }

  pub(super) fn appended(&self) -> &[RunEvent] {
    &self.events
  }

  /// The record's rules that hold before anything is read: the events are consecutive events of its run that end in
  /// the state it stores, and what is kept with them belongs to them.
  pub(super) fn check(&self) -> Result<(), RunStoreError> {
    let last = self
      .events
      .last()
      .ok_or(RunStoreError::EventsMismatch("there are no events to append"))?;
    if self.events.iter().any(|event| event.run() != self.change.run) {
      return Err(RunStoreError::EventsMismatch("an event belongs to another run"));
    }
    let consecutive = self
      .events
      .windows(2)
      .all(|pair| pair[0].sequence().next() == pair[1].sequence());
    if !consecutive {
      return Err(RunStoreError::EventsMismatch("the events are not consecutive"));
    }
    if last.sequence() != self.change.last_sequence {
      return Err(RunStoreError::EventsMismatch(
        "the last event does not produce the stored state",
      ));
    }
    match &self.with {
      KeptWith::Plan { plan } if plan.run != self.change.run => {
        Err(RunStoreError::EventsMismatch("the plan belongs to another run"))
      }
      KeptWith::RunnerFailure { .. } if !interrupts_after_runner_failure(last) => Err(RunStoreError::EventsMismatch(
        "the runner's failure is kept only with its interruption",
      )),
      _ => Ok(()),
    }
  }

  /// Stores the run's change and what is kept with the events, in `transaction`; the events themselves are the
  /// caller's.
  pub(super) fn store_change(&self, transaction: &Transaction<'_>) -> Result<(), StoreError> {
    self.change.update_run(transaction)?;
    self.change.write_attempts(transaction)
  }

  pub(super) fn store_kept(&self, transaction: &Transaction<'_>) -> Result<(), StoreError> {
    match &self.with {
      KeptWith::Nothing => Ok(()),
      KeptWith::Plan { plan } => super::plans::save(transaction, plan),
      KeptWith::RunnerFailure { failure } => {
        let interruption = self.events.last().expect("a checked record has events").sequence();
        super::super::runner_failures::insert(transaction, self.change.run, interruption, *failure)
      }
    }
  }
}

fn interrupts_after_runner_failure(event: &RunEvent) -> bool {
  matches!(
    event.kind(),
    RunEventKind::Interrupted {
      cause: InterruptCause::RunnerFailed
    }
  )
}

/// The columns appending events writes of a run's rows, as the store writes them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RunChange {
  run: RunId,
  task: TaskId,
  status: String,
  status_reason: Option<String>,
  failed_step: Option<String>,
  limit_execution: Option<String>,
  failure_reason: Option<String>,
  budget_tokens: Option<i64>,
  max_duration_ms: Option<i64>,
  used_tokens: i64,
  pause_requested: bool,
  last_sequence: EventSequence,
  branch: String,
  /// `(step_key, budget_tokens)` of each step budget raised for the run.
  step_budgets: Vec<(String, Option<i64>)>,
  /// `(profile_id, body)` of each profile whose permissions were changed for the run.
  permissions: Vec<(String, String)>,
  attempts: Vec<AttemptRow>,
}

/// An `attempts` row; an existing one only takes the new `status`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct AttemptRow {
  id: String,
  number: u32,
  reason: String,
  failed_step: Option<String>,
  return_to: Option<String>,
  note: Option<String>,
  status: String,
  conflicts: Option<String>,
  profile: Option<String>,
  executions: Vec<ExecutionRow>,
}

/// A `step_executions` row; an existing one only takes the new `status`, `wait_reason`, `failure` and
/// `judged_revision`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ExecutionRow {
  id: String,
  step_key: String,
  profile_version: Option<String>,
  model: Option<String>,
  status: String,
  wait_reason: Option<String>,
  failure: Option<String>,
  judged_revision: Option<String>,
}

impl RunChange {
  fn of(run: &Run) -> Result<Self, StoreError> {
    let columns = run_status_columns(run.status());
    let step_budgets = run
      .raised_step_budgets()
      .iter()
      .map(|(step, budget)| {
        Ok((
          step.as_str().to_owned(),
          optional_budget_column("step budget", *budget)?,
        ))
      })
      .collect::<Result<_, StoreError>>()?;
    let permissions = run
      .all_changed_permissions()
      .iter()
      .map(|(profile, changed)| {
        Ok((
          profile.to_string(),
          encode_json("run permissions", PERMISSIONS_SCHEMA, changed)?,
        ))
      })
      .collect::<Result<_, StoreError>>()?;
    Ok(Self {
      run: run.id(),
      task: run.task(),
      status: columns.status.to_owned(),
      status_reason: columns.reason.map(str::to_owned),
      failed_step: columns.failed_step.map(str::to_owned),
      limit_execution: columns.limit_execution,
      failure_reason: columns.failure_reason,
      budget_tokens: optional_budget_column("run budget", run.budget())?,
      max_duration_ms: optional_millis_column("run time limit", run.max_duration())?,
      used_tokens: tokens_column("run usage", run.used().get())?,
      pause_requested: run.pause_requested(),
      last_sequence: run.last_sequence(),
      branch: run.branch().as_str().to_owned(),
      step_budgets,
      permissions,
      attempts: attempt_rows(run)?,
    })
  }

  fn update_run(&self, transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute(
      "UPDATE runs SET status = ?2, status_reason = ?3, failed_step = ?4, limit_execution = ?5, budget_tokens = ?6, \
       max_duration_ms = ?7, used_tokens = ?8, pause_requested = ?9, last_sequence = ?10, branch = ?11, \
       failure_reason = ?12 WHERE id = ?1",
      params![
        self.run.to_string(),
        self.status,
        self.status_reason,
        self.failed_step,
        self.limit_execution,
        self.budget_tokens,
        self.max_duration_ms,
        self.used_tokens,
        self.pause_requested,
        sequence_column(self.last_sequence.get())?,
        self.branch,
        self.failure_reason,
      ],
    )?;
    let mut step_budget = transaction.prepare_cached(
      "INSERT INTO run_step_budgets (run_id, step_key, budget_tokens) VALUES (?1, ?2, ?3) \
       ON CONFLICT (run_id, step_key) DO UPDATE SET budget_tokens = excluded.budget_tokens",
    )?;
    for (step, budget) in &self.step_budgets {
      step_budget.execute(params![self.run.to_string(), step, budget])?;
    }
    let mut permissions = transaction.prepare_cached(
      "INSERT INTO run_permissions (run_id, profile_id, body) VALUES (?1, ?2, ?3)      ON CONFLICT (run_id, profile_id) DO UPDATE SET body = excluded.body",
    )?;
    for (profile, body) in &self.permissions {
      permissions.execute(params![self.run.to_string(), profile, body])?;
    }
    Ok(())
  }

  /// Inserts new attempts and executions and updates the status of existing ones.
  fn write_attempts(&self, transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let mut attempt_statement = transaction.prepare_cached(
      "INSERT INTO attempts (id, run_id, number, reason, failed_step, return_to, note, status, conflicts, profile) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ON CONFLICT (id) DO UPDATE SET status = excluded.status",
    )?;
    let mut execution_statement = transaction.prepare_cached(
      "INSERT INTO step_executions (id, attempt_id, step_key, profile_version_id, model, status, wait_reason, \
       failure, judged_revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
       ON CONFLICT (id) DO UPDATE SET status = excluded.status, wait_reason = excluded.wait_reason, \
       failure = excluded.failure, judged_revision = excluded.judged_revision",
    )?;
    for attempt in &self.attempts {
      attempt_statement.execute(params![
        attempt.id,
        self.run.to_string(),
        attempt.number,
        attempt.reason,
        attempt.failed_step,
        attempt.return_to,
        attempt.note,
        attempt.status,
        attempt.conflicts,
        attempt.profile,
      ])?;
      for execution in &attempt.executions {
        execution_statement.execute(params![
          execution.id,
          attempt.id,
          execution.step_key,
          execution.profile_version,
          execution.model,
          execution.status,
          execution.wait_reason,
          execution.failure,
          execution.judged_revision,
        ])?;
      }
    }
    Ok(())
  }
}

fn attempt_rows(run: &Run) -> Result<Vec<AttemptRow>, StoreError> {
  run
    .attempts()
    .iter()
    .map(|attempt| {
      let reason = attempt_reason_columns(attempt.reason());
      let executions = attempt
        .executions()
        .iter()
        .map(|execution| {
          // The profile the execution runs with: the one a return chose for it, else the snapshot's.
          let (profile_version, model) = match run.executing_agent(execution.id()) {
            Some((profile, model)) => (Some(profile.version.to_string()), Some(model.as_str().to_owned())),
            None => (None, None),
          };
          let (status, wait_reason) = step_status_columns(execution.status());
          ExecutionRow {
            id: execution.id().to_string(),
            step_key: execution.step().as_str().to_owned(),
            profile_version,
            model,
            status: status.to_owned(),
            wait_reason: wait_reason.map(str::to_owned),
            failure: execution.failure().map(|failure| step_failure_text(failure).to_owned()),
            judged_revision: execution.judged().map(|revision| revision.as_str().to_owned()),
          }
        })
        .collect();
      Ok(AttemptRow {
        id: attempt.id().to_string(),
        number: attempt.number().get(),
        reason: reason.reason.to_owned(),
        failed_step: reason.failed_step.map(str::to_owned),
        return_to: reason.return_to.map(str::to_owned),
        note: reason.note.map(str::to_owned),
        status: attempt_status_text(attempt.status()).to_owned(),
        conflicts: attempt_conflicts_column(attempt.reason())?,
        profile: attempt_profile_column(attempt.reason())?,
        executions,
      })
    })
    .collect()
}
