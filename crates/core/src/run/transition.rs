//! Run-level transitions: pending → running ⇄ paused → succeeded / failed / cancelled;
//! running → interrupted → running on continue; running → stopped at a limit → running once the
//! limit is raised. Each returns the events to append.

use std::time::Duration;

use super::{
  attempt::{Checkpoint, StepExecution},
  branch::BranchName,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  not_started::WorktreeFailure,
  state::Run,
  status::{AttemptReason, ContinueTrigger, InterruptCause, PauseReason, RunFailure, RunLimit, RunStatus, StepStatus},
};
use crate::{
  id::{AttemptId, CheckpointId, StepExecutionId},
  revision::Revision,
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};

/// New limits for a run stopped at a limit; they apply to that run only. `None` lifts a limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RaisedLimits {
  pub budget: Option<TokenBudget>,
  pub max_duration: Option<Duration>,
}

impl Run {
  /// Starts a pending run with attempt 1, which executes every step.
  pub fn start(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::Started)?;
      let executions = transition
        .run
        .snapshot()
        .steps()
        .iter()
        .map(|step| PlannedExecution {
          id: StepExecutionId::generate(),
          step: step.key().clone(),
        })
        .collect();
      transition.record(RunEventKind::AttemptStarted {
        attempt: AttemptId::generate(),
        number: transition.run.next_attempt_number(),
        reason: AttemptReason::Initial,
        executions,
      })
    })
  }

  /// Records that the pending run works on `branch`, where its worktree was made because a branch
  /// of the run's own name existed already. Its own name changes nothing.
  pub fn choose_branch(&mut self, branch: BranchName, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    if branch == *self.branch() {
      return Ok(Vec::new());
    }
    self.transition(now, |transition| {
      transition.record(RunEventKind::BranchChosen { branch })
    })
  }

  /// Fails the pending run, whose worktree could not be created for `reason`: it never started, and
  /// works again only as a new run.
  pub fn fail_to_start(&mut self, reason: WorktreeFailure, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::Failed {
        reason: RunFailure::WorktreeNotCreated(reason),
      })
    })
  }

  /// Records that a person asked to pause. The run keeps working until its active step reaches
  /// a checkpoint, then the runner records [`Run::pause`]. Asking again changes nothing.
  pub fn request_pause(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    if self.pause_requested && self.status == RunStatus::Running {
      return Ok(Vec::new());
    }
    self.transition(now, |transition| transition.record(RunEventKind::PauseRequested))
  }

  /// Records that the run settled at a safe checkpoint and stopped scheduling. The runner calls
  /// this only after active steps reached their checkpoint.
  pub fn pause(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::Paused {
        reason: PauseReason::Requested,
      })
    })
  }

  /// Records that the run paused because the disk ran low: its agents were ended and their work committed, so it
  /// waits with nothing at work and goes on with a plain [`Run::resume`] once the disk has room.
  pub fn pause_for_low_disk(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::Paused {
        reason: PauseReason::LowDisk,
      })
    })
  }

  /// Records that the run reached `limit`: its agent was ended and its work committed as a
  /// checkpoint, so the run waits as "Stopped at limit" until the limit is raised.
  pub fn stop_at_limit(&mut self, limit: RunLimit, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::Paused {
        reason: PauseReason::LimitReached(limit),
      })
    })
  }

  /// Raises the limits of a run stopped at a limit of its own (tokens, time) and continues it. Each
  /// new limit must be above what the run already used (`active` agent time so far); `None` lifts
  /// the limit. There is no maximum.
  pub fn continue_with_limits(
    &mut self,
    limits: RaisedLimits,
    active: Duration,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    if limits.max_duration.is_some_and(|limit| limit <= active) {
      return Err(RunError::LimitNotAboveUsed(RunLimit::Time));
    }
    if crate::limits::too_long(limits.max_duration) {
      return Err(RunError::TimeLimitTooLong);
    }
    self.transition(now, |transition| {
      transition.record(RunEventKind::LimitRaised {
        budget: limits.budget,
        max_duration: limits.max_duration,
      })?;
      transition.record(RunEventKind::Resumed)
    })
  }

  /// Raises the budget of the step a run stopped at to `budget` for this run, or lifts it with
  /// `None`, and continues it; the published pipeline keeps its budget. `used` is what the stopped
  /// execution used over all its invocations: a new budget must be above it.
  pub fn raise_step_budget(
    &mut self,
    budget: Option<TokenBudget>,
    used: TokensUsed,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    let RunStatus::Paused(PauseReason::LimitReached(limit @ RunLimit::StepBudget { execution })) = self.status else {
      return Err(RunError::InvalidRunTransition {
        status: self.status.clone(),
        event: "step_budget_raised",
      });
    };
    if budget.is_some_and(|budget| used.reached(budget)) {
      return Err(RunError::LimitNotAboveUsed(limit));
    }
    let step = self
      .execution(execution)
      .expect("a restored or replayed run stopped at the budget of one of its executions")
      .step()
      .clone();
    self.transition(now, |transition| {
      transition.record(RunEventKind::StepBudgetRaised { step, budget })?;
      transition.record(RunEventKind::Resumed)
    })
  }

  /// Continues a paused run with the same snapshot. A run stopped at a limit continues only
  /// through [`Run::continue_with_limits`].
  pub fn resume(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| transition.record(RunEventKind::Resumed))
  }

  /// Records that a running run stopped without settling. Its last checkpoint is kept; step
  /// executions keep their status and nothing after the checkpoint counts as done.
  pub fn interrupt(&mut self, cause: InterruptCause, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| transition.record(RunEventKind::Interrupted { cause }))
  }

  /// Continues an interrupted run from its last checkpoint because a person asked to.
  pub fn continue_run(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.continue_from_checkpoint(ContinueTrigger::User, now)
  }

  /// Whether a reconnecting runner continues this run without asking.
  pub fn continues_on_reconnect(&self) -> bool {
    matches!(self.status, RunStatus::Interrupted(cause) if cause.continues_on_reconnect())
  }

  /// Continues the run from its last checkpoint after the runner reconnected, if the runner's
  /// loss interrupted it. A run that is no longer interrupted has nothing to continue, so a
  /// repeated call returns no events. A run interrupted by the end of a session is rejected:
  /// it waits for an explicit [`Run::continue_run`].
  pub fn continue_after_reconnect(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    match self.status {
      RunStatus::Interrupted(_) => self.continue_from_checkpoint(ContinueTrigger::RunnerReconnected, now),
      _ => Ok(Vec::new()),
    }
  }

  fn continue_from_checkpoint(&mut self, trigger: ContinueTrigger, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    let from = self.last_checkpoint().map(Checkpoint::id);
    self.transition(now, |transition| {
      transition.record(RunEventKind::Continued { trigger, from })
    })
  }

  /// Stops the run for good. Open step executions are skipped; completed work and artifacts
  /// stay as they are. A pending run ends without an attempt.
  pub fn cancel(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.skip_open()?;
      transition.record(RunEventKind::Cancelled)
    })
  }

  /// Records that the run's worktree was committed at `revision` during `execution`.
  pub fn record_checkpoint(
    &mut self,
    execution: StepExecutionId,
    revision: Revision,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::CheckpointRecorded {
        checkpoint: CheckpointId::generate(),
        execution,
        revision,
      })
    })
  }

  /// Records `revision`, the tip of a succeeded run's branch, as the checkpoint of its last passed execution, when the
  /// run has none: its agents committed their work themselves, which runners before ORC-147 did not record.
  pub fn record_missing_checkpoint(&mut self, revision: Revision, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    let execution = self
      .current_attempt()
      .and_then(|attempt| {
        attempt
          .executions()
          .iter()
          .rev()
          .find(|execution| execution.status() == StepStatus::Passed)
      })
      .map(StepExecution::id)
      .ok_or(RunError::StepsNotPassed)?;
    self.record_checkpoint(execution, revision, now)
  }

  /// Adds tokens an agent CLI reported. Usage is a fact, so it is recorded in every status.
  pub fn record_usage(&mut self, tokens: TokensUsed, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::UsageRecorded { tokens })
    })
  }
}
