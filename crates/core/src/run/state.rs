use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
  sync::Arc,
  time::Duration,
};

use super::{
  attempt::{Attempt, Checkpoint, StepExecution},
  branch::BranchName,
  error::RunError,
  event::{EventSequence, RunCreated, RunEvent, RunEventKind},
  number::RunNumber,
  permissions::RunPermissions,
  snapshot::RunSnapshot,
  status::{AttemptReason, PauseReason, RunFailure, RunLimit, RunStatus, StepStatus},
};
use crate::{
  id::{ResourceId, RunId, StepExecutionId, TaskId},
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};

/// What a caller decides when it starts a run; the store adds the id and the display number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewRun {
  pub task: TaskId,
  pub snapshot: RunSnapshot,
  pub branch: BranchName,
  pub base: Revision,
  /// The branch the user's checkout had at `base`; `None` on a detached HEAD.
  pub base_branch: Option<BranchName>,
  /// `None`: the run has no token budget.
  pub budget: Option<TokenBudget>,
  /// Most active agent time the run may use; `None`: no time limit.
  pub max_duration: Option<Duration>,
}

/// Current state of a run as persisted alongside its events, before [`Run::restore`] checks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredRun {
  pub id: RunId,
  pub created: RunCreated,
  pub created_at: Timestamp,
  pub status: RunStatus,
  /// Limits in force, the creation limits or the last raised ones; `None` is no limit.
  pub budget: Option<TokenBudget>,
  pub max_duration: Option<Duration>,
  /// Step budgets raised for this run, in force instead of the snapshot's; `None` lifted the
  /// step's budget.
  pub step_budgets: BTreeMap<StepKey, Option<TokenBudget>>,
  /// Permissions changed for this run, by agent profile.
  pub permissions: BTreeMap<ResourceId, RunPermissions>,
  pub used: TokensUsed,
  pub pause_requested: bool,
  pub attempts: Vec<Attempt>,
  pub last_checkpoint: Option<Checkpoint>,
  pub last_sequence: EventSequence,
}

/// One execution of a task: its pinned configuration, attempts, step executions, last
/// checkpoint, limits and usage. Changes go through transition methods, each returning the events
/// to append; applying the same events to the creation event rebuilds the same state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
  pub(super) id: RunId,
  pub(super) created: Arc<RunCreated>,
  pub(super) created_at: Timestamp,
  pub(super) status: RunStatus,
  pub(super) budget: Option<TokenBudget>,
  pub(super) max_duration: Option<Duration>,
  pub(super) step_budgets: BTreeMap<StepKey, Option<TokenBudget>>,
  pub(super) permissions: BTreeMap<ResourceId, RunPermissions>,
  pub(super) used: TokensUsed,
  pub(super) pause_requested: bool,
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
      base_branch: new.base_branch,
      budget: new.budget,
      max_duration: new.max_duration,
    };
    let run = Self::from_created(id, created.clone(), now)?;
    let event = RunEvent::new(id, EventSequence::FIRST, now, RunEventKind::Created(Box::new(created)));
    Ok((run, event))
  }

  pub(super) fn from_created(id: RunId, created: RunCreated, at: Timestamp) -> Result<Self, RunError> {
    if zero(created.max_duration) {
      return Err(RunError::ZeroDuration);
    }
    if crate::limits::too_long(created.max_duration) {
      return Err(RunError::TimeLimitTooLong);
    }
    Ok(Self {
      id,
      budget: created.budget,
      max_duration: created.max_duration,
      step_budgets: BTreeMap::new(),
      permissions: BTreeMap::new(),
      used: TokensUsed::ZERO,
      pause_requested: false,
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
    if zero(stored.max_duration) {
      return Err(RunError::ZeroDuration);
    }
    run.status = stored.status;
    run.budget = stored.budget;
    run.max_duration = stored.max_duration;
    run.step_budgets = stored.step_budgets;
    run.permissions = stored.permissions;
    run.used = stored.used;
    run.pause_requested = stored.pause_requested;
    run.attempts = stored.attempts;
    run.last_checkpoint = stored.last_checkpoint;
    run.last_sequence = stored.last_sequence;
    run.check_restored()?;
    Ok(run)
  }

  fn check_restored(&self) -> Result<(), RunError> {
    if self.pause_requested && self.status != RunStatus::Running {
      return Err(RunError::Inconsistent("only a running run waits to pause"));
    }
    if self.status == RunStatus::Pending && !self.attempts.is_empty() {
      return Err(RunError::Inconsistent("a pending run has no attempts"));
    }
    // Cancelling a pending run records no attempt, nor does failing one whose worktree could not be
    // created: each ends before it started.
    let started = !self.attempts.is_empty();
    let consistent = match &self.status {
      RunStatus::Pending | RunStatus::Failed(RunFailure::WorktreeNotCreated(_)) => !started,
      RunStatus::Cancelled => true,
      _ => started,
    };
    if !consistent {
      return Err(RunError::Inconsistent(
        "only a pending run, or one cancelled or failed before it started, has no attempts",
      ));
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
    if self
      .step_budgets
      .keys()
      .any(|step| self.snapshot().step(step).is_none())
    {
      return Err(RunError::Inconsistent(
        "a raised step budget names a step outside the snapshot",
      ));
    }
    self.check_restored_permissions()?;
    if let RunStatus::Paused(PauseReason::LimitReached(RunLimit::StepBudget { execution })) = self.status
      && self.execution(execution).is_none()
    {
      return Err(RunError::Inconsistent(
        "the run stopped at the budget of an unknown execution",
      ));
    }
    if self
      .executions()
      .any(|execution| execution.failure().is_some() && execution.status() != StepStatus::Failed)
    {
      return Err(RunError::Inconsistent("only a failed execution has a failure reason"));
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

  /// The branch the run works on: the one it was created with, or the free one it was moved to
  /// ([`Run::choose_branch`]).
  pub fn branch(&self) -> &BranchName {
    &self.created.branch
  }

  /// The branch that is the run's own, which its leftovers, cleanup, pull requests and screens name: `None` for a
  /// run that never started as its worktree could not be created. It made no branch, and a branch of the name it was
  /// created with is someone else's.
  pub fn own_branch(&self) -> Option<&BranchName> {
    (!matches!(self.status, RunStatus::Failed(RunFailure::WorktreeNotCreated(_)))).then_some(&self.created.branch)
  }

  /// Revision the run's branch was created from.
  pub fn base(&self) -> &Revision {
    &self.created.base
  }

  /// The branch the user's checkout had when the run started: where merging the run brings its
  /// work. `None` when it had a detached HEAD, or the run was recorded before Orchester kept it.
  pub fn base_branch(&self) -> Option<&BranchName> {
    self.created.base_branch.as_ref()
  }

  /// The token budget in force: the creation budget or the last raised one; `None` when the run
  /// has none.
  pub const fn budget(&self) -> Option<TokenBudget> {
    self.budget
  }

  /// The time limit on active agent time in force; `None` when the run has none.
  pub const fn max_duration(&self) -> Option<Duration> {
    self.max_duration
  }

  /// Tokens used so far, as the CLIs reported them.
  pub const fn used(&self) -> TokensUsed {
    self.used
  }

  /// The budget in force for each execution of `step`: the one raised for this run, else the
  /// snapshot's; `None` when the step has none.
  pub fn step_budget(&self, step: &StepKey) -> Option<TokenBudget> {
    match self.step_budgets.get(step) {
      Some(raised) => *raised,
      None => self.snapshot().body().step(step).and_then(|step| step.budget),
    }
  }

  /// Step budgets raised for this run; `None` lifted the step's budget.
  pub const fn raised_step_budgets(&self) -> &BTreeMap<StepKey, Option<TokenBudget>> {
    &self.step_budgets
  }

  /// Permissions changed for this run, by agent profile.
  pub const fn all_changed_permissions(&self) -> &BTreeMap<ResourceId, RunPermissions> {
    &self.permissions
  }

  /// Whether the used tokens have reached the budget; never for a run without one.
  pub const fn token_limit_reached(&self) -> bool {
    match self.budget {
      Some(budget) => self.used.reached(budget),
      None => false,
    }
  }

  /// Whether `active` agent time has reached the time limit; never for a run without one.
  pub fn time_limit_reached(&self, active: Duration) -> bool {
    self.max_duration.is_some_and(|limit| active >= limit)
  }

  /// A person asked to pause and the run is settling at its next checkpoint ("Pausing…").
  pub const fn pause_requested(&self) -> bool {
    self.pause_requested
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

  /// The step's most recent execution: the one that decides whether it has passed. A follow-up that
  /// failed decides nothing ([`Attempt::counts`]).
  pub fn latest_execution(&self, step: &StepKey) -> Option<&StepExecution> {
    self
      .attempts
      .iter()
      .rev()
      .filter(|attempt| attempt.counts())
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

/// A time limit of zero, which no run may have.
fn zero(limit: Option<Duration>) -> bool {
  limit.is_some_and(|limit| limit.is_zero())
}
