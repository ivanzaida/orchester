use std::{collections::BTreeSet, num::NonZeroU32, sync::Arc};

use super::{
  attempt::{Attempt, Checkpoint, StepExecution},
  branch::BranchName,
  error::RunError,
  event::{EventSequence, RunCreated, RunEvent, RunEventKind},
  number::RunNumber,
  snapshot::RunSnapshot,
  status::{AttemptReason, RunStatus, StepStatus},
};
use crate::{
  id::{RunId, StepExecutionId, TaskId},
  money::Money,
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
};

/// What a caller decides when it starts a run; the store adds the id and the display number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRun {
  pub task: TaskId,
  pub snapshot: RunSnapshot,
  pub branch: BranchName,
  pub base: Revision,
  pub budget: Money,
}

/// Current state of a run as persisted alongside its events, before [`Run::restore`] checks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredRun {
  pub id: RunId,
  pub created: RunCreated,
  pub created_at: Timestamp,
  pub status: RunStatus,
  pub spent: Money,
  pub attempts: Vec<Attempt>,
  pub last_checkpoint: Option<Checkpoint>,
  pub last_sequence: EventSequence,
}

/// One execution of a task: its pinned configuration, attempts, step executions, last
/// checkpoint and spending. Changes go through transition methods, each returning the events
/// to append; applying the same events to the creation event rebuilds the same state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
  pub(super) id: RunId,
  pub(super) created: Arc<RunCreated>,
  pub(super) created_at: Timestamp,
  pub(super) status: RunStatus,
  pub(super) spent: Money,
  pub(super) attempts: Vec<Attempt>,
  pub(super) last_checkpoint: Option<Checkpoint>,
  pub(super) last_sequence: EventSequence,
}

impl Run {
  /// A pending run and its creation event, the first of its log.
  pub fn create(id: RunId, number: RunNumber, new: NewRun, now: Timestamp) -> Result<(Self, RunEvent), RunError> {
    let created = RunCreated {
      task: new.task,
      number,
      snapshot: new.snapshot,
      branch: new.branch,
      base: new.base,
      budget: new.budget,
    };
    let run = Self::from_created(id, created.clone(), now)?;
    let event = RunEvent::new(id, EventSequence::FIRST, now, RunEventKind::Created(Box::new(created)));
    Ok((run, event))
  }

  pub(super) fn from_created(id: RunId, created: RunCreated, at: Timestamp) -> Result<Self, RunError> {
    if created.budget.micros() <= 0 {
      return Err(RunError::NonPositiveBudget);
    }
    Ok(Self {
      id,
      spent: Money::from_micros(0, created.budget.currency()),
      created: Arc::new(created),
      created_at: at,
      status: RunStatus::Pending,
      attempts: Vec::new(),
      last_checkpoint: None,
      last_sequence: EventSequence::FIRST,
    })
  }

  /// Rebuilds a persisted run, rejecting state that no event log could have produced.
  pub fn restore(stored: StoredRun) -> Result<Self, RunError> {
    let mut run = Self::from_created(stored.id, stored.created, stored.created_at)?;
    run.status = stored.status;
    run.spent = stored.spent;
    run.attempts = stored.attempts;
    run.last_checkpoint = stored.last_checkpoint;
    run.last_sequence = stored.last_sequence;
    run.check_restored()?;
    Ok(run)
  }

  fn check_restored(&self) -> Result<(), RunError> {
    if self.spent.currency() != self.budget().currency() {
      return Err(RunError::Inconsistent("spent and budget currencies differ"));
    }
    if (self.status == RunStatus::Pending) != self.attempts.is_empty() {
      return Err(RunError::Inconsistent("only a pending run has no attempts"));
    }
    let numbered = (1..)
      .zip(&self.attempts)
      .all(|(n, attempt)| attempt.number().get() == n);
    if !numbered {
      return Err(RunError::Inconsistent("attempts are not numbered 1..n"));
    }
    let known = self
      .executions()
      .all(|execution| self.snapshot().step(execution.step()).is_some());
    if !known {
      return Err(RunError::Inconsistent("an execution names a step outside the snapshot"));
    }
    match &self.last_checkpoint {
      Some(checkpoint) if self.execution(checkpoint.execution()).is_none() => {
        Err(RunError::Inconsistent("the last checkpoint names an unknown execution"))
      }
      _ => Ok(()),
    }
  }

  pub const fn id(&self) -> RunId {
    self.id
  }

  pub fn task(&self) -> TaskId {
    self.created.task
  }

  pub fn number(&self) -> RunNumber {
    self.created.number
  }

  pub fn snapshot(&self) -> &RunSnapshot {
    &self.created.snapshot
  }

  pub fn branch(&self) -> &BranchName {
    &self.created.branch
  }

  /// Revision the run's branch was created from.
  pub fn base(&self) -> &Revision {
    &self.created.base
  }

  pub fn budget(&self) -> Money {
    self.created.budget
  }

  /// Creation facts, as recorded in the first event.
  pub fn created(&self) -> &RunCreated {
    &self.created
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }

  pub const fn status(&self) -> &RunStatus {
    &self.status
  }

  pub const fn spent(&self) -> Money {
    self.spent
  }

  pub fn attempts(&self) -> &[Attempt] {
    &self.attempts
  }

  pub fn current_attempt(&self) -> Option<&Attempt> {
    self.attempts.last()
  }

  pub const fn last_checkpoint(&self) -> Option<&Checkpoint> {
    self.last_checkpoint.as_ref()
  }

  /// Sequence of the last event applied to this state.
  pub const fn last_sequence(&self) -> EventSequence {
    self.last_sequence
  }

  pub fn execution(&self, id: StepExecutionId) -> Option<&StepExecution> {
    self.executions().find(|execution| execution.id() == id)
  }

  /// The step's most recent execution: the one that decides whether it has passed.
  pub fn latest_execution(&self, step: &StepKey) -> Option<&StepExecution> {
    self
      .attempts
      .iter()
      .rev()
      .find_map(|attempt| attempt.executions().iter().find(|execution| execution.step() == step))
  }

  /// How often the failure route of `step` has been taken.
  pub fn retries_taken(&self, step: &StepKey) -> u32 {
    let taken = self
      .attempts
      .iter()
      .filter(|attempt| matches!(attempt.reason(), AttemptReason::Retry { failed_step, .. } if failed_step == step));
    u32::try_from(taken.count()).expect("fewer than 2^32 attempts")
  }

  fn executions(&self) -> impl Iterator<Item = &StepExecution> {
    self.attempts.iter().flat_map(Attempt::executions)
  }

  pub(super) fn all_passed(&self) -> bool {
    self
      .snapshot()
      .steps()
      .iter()
      .all(|step| self.latest_execution(step.key()).map(StepExecution::status) == Some(StepStatus::Passed))
  }

  pub(super) fn has_open_executions(&self) -> bool {
    self.current_attempt().is_some_and(|attempt| {
      attempt
        .executions()
        .iter()
        .any(|execution| execution.status().is_open())
    })
  }

  pub(super) fn next_attempt_number(&self) -> NonZeroU32 {
    let count = u32::try_from(self.attempts.len()).expect("fewer than 2^32 attempts");
    NonZeroU32::MIN.saturating_add(count)
  }

  pub(super) fn step_keys(&self) -> BTreeSet<&StepKey> {
    self.snapshot().steps().iter().map(|step| step.key()).collect()
  }
}
