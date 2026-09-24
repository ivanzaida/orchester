//! Run-level transitions: pending → running ⇄ paused → succeeded / failed / cancelled;
//! running → interrupted → running on continue. Each returns the events to append.

use super::{
  attempt::Checkpoint,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  state::Run,
  status::{AttemptReason, ContinueTrigger, InterruptCause, RunStatus},
};
use crate::{
  id::{AttemptId, CheckpointId, StepExecutionId},
  money::Money,
  revision::Revision,
  time::Timestamp,
};

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

  /// Records that the run settled at a safe checkpoint and stopped scheduling. The runner calls
  /// this only after active steps reached their checkpoint.
  pub fn pause(&mut self, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| transition.record(RunEventKind::Paused))
  }

  /// Continues a paused run with the same snapshot.
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
  /// stay as they are.
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

  /// Adds money the run spent. Spending is a fact, so it is recorded in every status; it
  /// must be in the budget's currency.
  pub fn record_spend(&mut self, amount: Money, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    self.transition(now, |transition| {
      transition.record(RunEventKind::SpendRecorded { amount })
    })
  }
}
