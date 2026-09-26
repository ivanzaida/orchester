//! The single place where events change a run. Transitions and replay both go through
//! [`Run::check`] and [`Run::apply`], so a replayed log reaches exactly the state the
//! transitions produced.

use std::{collections::BTreeSet, num::NonZeroU32};

use super::{
  attempt::{Attempt, Checkpoint, StepExecution},
  error::RunError,
  event::{EventSequence, PlannedExecution, RunEvent, RunEventKind},
  state::Run,
  status::{
    AttemptReason, AttemptStatus, ContinueTrigger, PauseReason, PlanDecision, RunLimit, RunStatus, StepStatus,
    WaitReason,
  },
};
use crate::{
  id::{CheckpointId, StepExecutionId},
  time::Timestamp,
  tokens::TokenBudget,
};

impl Run {
  /// Rebuilds a run from its complete event log.
  pub fn replay(events: &[RunEvent]) -> Result<Self, RunError> {
    let (first, rest) = events.split_first().ok_or(RunError::Creation)?;
    let RunEventKind::Created(created) = first.kind() else {
      return Err(RunError::Creation);
    };
    if first.sequence() != EventSequence::FIRST {
      return Err(RunError::Creation);
    }
    let mut run = Self::from_created(first.run(), (**created).clone(), first.at())?;
    for event in rest {
      if event.run() != run.id {
        return Err(RunError::WrongRun {
          expected: run.id,
          found: event.run(),
        });
      }
      if event.sequence() != run.last_sequence.next() {
        return Err(RunError::OutOfSequence {
          previous: run.last_sequence.get(),
          found: event.sequence().get(),
        });
      }
      run.record(event.kind().clone(), event.at())?;
    }
    Ok(run)
  }

  /// Applies a group of events atomically: either every event recorded by `build` is accepted
  /// and the run changes, or the run stays as it was.
  pub(super) fn transition(
    &mut self,
    at: Timestamp,
    build: impl FnOnce(&mut Transition) -> Result<(), RunError>,
  ) -> Result<Vec<RunEvent>, RunError> {
    let mut transition = Transition {
      run: self.clone(),
      at,
      events: Vec::new(),
    };
    build(&mut transition)?;
    *self = transition.run;
    Ok(transition.events)
  }

  fn record(&mut self, kind: RunEventKind, at: Timestamp) -> Result<RunEvent, RunError> {
    self.check(&kind)?;
    self.apply(&kind, at);
    self.last_sequence = self.last_sequence.next();
    Ok(RunEvent::new(self.id, self.last_sequence, at, kind))
  }

  fn check(&self, kind: &RunEventKind) -> Result<(), RunError> {
    match kind {
      RunEventKind::Created(_) => Err(RunError::Creation),
      RunEventKind::Started => self.expect_status(kind, matches!(self.status, RunStatus::Pending)),
      RunEventKind::PauseRequested => {
        self.expect_status(kind, self.status == RunStatus::Running && !self.pause_requested)
      }
      RunEventKind::Paused { .. } | RunEventKind::Interrupted { .. } => {
        self.expect_status(kind, self.status == RunStatus::Running)
      }
      RunEventKind::Resumed => match self.status {
        // The daily project budget is shared; a run stopped by it continues once the day has
        // room again, which the runner checks, not with limits of its own.
        RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(RunLimit::DailyBudget)) => Ok(()),
        RunStatus::Paused(PauseReason::LimitReached(limit)) => Err(RunError::LimitNotRaised(limit)),
        _ => self.expect_status(kind, false),
      },
      RunEventKind::LimitRaised { budget, .. } => self.check_raise(kind, *budget),
      RunEventKind::Continued { trigger, from } => self.check_continue(kind, *trigger, *from),
      RunEventKind::Succeeded => {
        self.expect_status(kind, self.status == RunStatus::Running)?;
        if self.all_passed() {
          Ok(())
        } else {
          Err(RunError::StepsNotPassed)
        }
      }
      RunEventKind::Failed { .. } => {
        self.expect_status(kind, self.status == RunStatus::Running)?;
        self.expect_settled()
      }
      RunEventKind::Cancelled => {
        self.expect_status(kind, !self.status.is_terminal())?;
        self.expect_settled()
      }
      RunEventKind::AttemptStarted {
        number,
        reason,
        executions,
        ..
      } => self.check_attempt(kind, *number, reason, executions),
      RunEventKind::UsageRecorded { .. } => Ok(()),
      _ => self.check_step(kind),
    }
  }

  fn check_step(&self, kind: &RunEventKind) -> Result<(), RunError> {
    use StepStatus::{Pending, Running, Waiting};
    let execution = kind.execution().expect("run-level events are checked by Run::check");
    if let RunEventKind::StepSkipped { .. } = kind {
      self.expect_status(kind, !self.status.is_terminal())?;
      return self.expect_step(kind, execution, StepStatus::is_open);
    }
    self.expect_status(kind, self.status == RunStatus::Running)?;
    match kind {
      RunEventKind::StepStarted { .. } => {
        self.expect_step(kind, execution, |status| status == Pending)?;
        self.expect_dependencies_passed(execution)
      }
      RunEventKind::StepWaiting { reason, .. } => {
        self.expect_step(kind, execution, |status| status == Running)?;
        if *reason == WaitReason::Plan {
          self.expect_gate(execution, true)
        } else {
          Ok(())
        }
      }
      RunEventKind::StepResumed { .. } => self.expect_step(kind, execution, |status| {
        matches!(status, Waiting(WaitReason::Approval | WaitReason::Message))
      }),
      RunEventKind::PlanDecided { .. } => {
        self.expect_step(kind, execution, |status| status == Waiting(WaitReason::Plan))
      }
      RunEventKind::StepPassed { .. } => {
        self.expect_step(kind, execution, |status| status == Running)?;
        self.expect_gate(execution, false)
      }
      // StepFailed and CheckpointRecorded: an active execution.
      _ => self.expect_step(kind, execution, |status| matches!(status, Running | Waiting(_))),
    }
  }

  fn expect_status(&self, kind: &RunEventKind, allowed: bool) -> Result<(), RunError> {
    if allowed {
      Ok(())
    } else {
      Err(RunError::InvalidRunTransition {
        status: self.status.clone(),
        event: kind.name(),
      })
    }
  }

  fn expect_settled(&self) -> Result<(), RunError> {
    if self.has_open_executions() {
      Err(RunError::OpenExecutions)
    } else {
      Ok(())
    }
  }

  /// The execution must be in the current attempt with a status `allowed` accepts.
  fn expect_step(
    &self,
    kind: &RunEventKind,
    id: StepExecutionId,
    allowed: impl Fn(StepStatus) -> bool,
  ) -> Result<(), RunError> {
    let execution = self.current_execution(id)?;
    if allowed(execution.status()) {
      Ok(())
    } else {
      Err(RunError::InvalidStepTransition {
        step: execution.step().clone(),
        status: execution.status(),
        event: kind.name(),
      })
    }
  }

  fn current_execution(&self, id: StepExecutionId) -> Result<&StepExecution, RunError> {
    if let Some(execution) = self.current_attempt().and_then(|attempt| attempt.execution(id)) {
      return Ok(execution);
    }
    match self.execution(id) {
      Some(_) => Err(RunError::NotInCurrentAttempt(id)),
      None => Err(RunError::UnknownExecution(id)),
    }
  }

  fn expect_dependencies_passed(&self, id: StepExecutionId) -> Result<(), RunError> {
    let key = self.current_execution(id)?.step();
    let step = self
      .snapshot()
      .body()
      .step(key)
      .expect("executions name snapshot steps");
    let passed = step
      .depends_on
      .iter()
      .all(|dependency| self.latest_execution(dependency).map(StepExecution::status) == Some(StepStatus::Passed));
    if passed {
      Ok(())
    } else {
      Err(RunError::DependenciesNotPassed(key.clone()))
    }
  }

  /// Plan waits and plan decisions belong to gated steps; plain passes to ungated ones.
  fn expect_gate(&self, id: StepExecutionId, gated: bool) -> Result<(), RunError> {
    let key = self.current_execution(id)?.step();
    let step = self
      .snapshot()
      .body()
      .step(key)
      .expect("executions name snapshot steps");
    match (step.approval_gate, gated) {
      (true, false) => Err(RunError::GatedStep(key.clone())),
      (false, true) => Err(RunError::NotGated(key.clone())),
      _ => Ok(()),
    }
  }

  fn check_continue(
    &self,
    kind: &RunEventKind,
    trigger: ContinueTrigger,
    from: Option<CheckpointId>,
  ) -> Result<(), RunError> {
    let RunStatus::Interrupted(cause) = self.status else {
      return self.expect_status(kind, false);
    };
    if trigger == ContinueTrigger::RunnerReconnected && !cause.continues_on_reconnect() {
      return Err(RunError::NeedsExplicitContinue(cause));
    }
    let expected = self.last_checkpoint.as_ref().map(Checkpoint::id);
    if from == expected {
      Ok(())
    } else {
      Err(RunError::CheckpointMismatch { expected, found: from })
    }
  }

  fn check_attempt(
    &self,
    kind: &RunEventKind,
    number: NonZeroU32,
    reason: &AttemptReason,
    executions: &[PlannedExecution],
  ) -> Result<(), RunError> {
    self.expect_status(kind, self.status == RunStatus::Running)?;
    self.expect_settled()?;
    if number != self.next_attempt_number() {
      return Err(RunError::InvalidAttempt("attempt numbers continue without gaps"));
    }
    let steps: BTreeSet<_> = executions.iter().map(|planned| &planned.step).collect();
    if steps.len() != executions.len() || executions.is_empty() {
      return Err(RunError::InvalidAttempt("an attempt executes each step at most once"));
    }
    let known = self.step_keys();
    if !steps.is_subset(&known) {
      return Err(RunError::InvalidAttempt("an attempt executes only snapshot steps"));
    }
    match (self.attempts.is_empty(), reason) {
      (true, AttemptReason::Initial) if steps == known => Ok(()),
      (true, _) => Err(RunError::InvalidAttempt("the first attempt executes every step")),
      (false, AttemptReason::Retry { .. }) => Ok(()),
      (false, AttemptReason::Initial) => Err(RunError::InvalidAttempt("only the first attempt is initial")),
    }
  }

  /// A raised limit applies to a run stopped at a limit, and its budget is above what the run
  /// used. The time limit is checked against active time by [`Run::continue_with_limits`],
  /// which knows it.
  fn check_raise(&self, kind: &RunEventKind, budget: TokenBudget) -> Result<(), RunError> {
    self.expect_status(
      kind,
      matches!(self.status, RunStatus::Paused(PauseReason::LimitReached(_))),
    )?;
    if self.used.reached(budget) {
      return Err(RunError::LimitNotAboveUsed(RunLimit::Tokens));
    }
    Ok(())
  }

  /// Changes the state for an event [`Run::check`] accepted.
  fn apply(&mut self, kind: &RunEventKind, at: Timestamp) {
    match kind {
      RunEventKind::Created(_) => unreachable!("Run::check rejects a second creation event"),
      RunEventKind::Started | RunEventKind::Resumed | RunEventKind::Continued { .. } => {
        self.status = RunStatus::Running;
      }
      RunEventKind::PauseRequested => self.pause_requested = true,
      RunEventKind::Paused { reason } => {
        self.status = RunStatus::Paused(*reason);
        self.pause_requested = false;
      }
      RunEventKind::Interrupted { cause } => {
        self.status = RunStatus::Interrupted(*cause);
        self.pause_requested = false;
      }
      // With its limits raised the run is an ordinary paused run, which resuming continues.
      RunEventKind::LimitRaised { budget, max_duration } => {
        self.budget = *budget;
        self.max_duration = *max_duration;
        self.status = RunStatus::Paused(PauseReason::Requested);
      }
      RunEventKind::Succeeded => self.end(RunStatus::Succeeded, AttemptStatus::Succeeded),
      RunEventKind::Failed { reason } => self.end(RunStatus::Failed(reason.clone()), AttemptStatus::Failed),
      RunEventKind::Cancelled => self.end(RunStatus::Cancelled, AttemptStatus::Cancelled),
      RunEventKind::AttemptStarted {
        attempt,
        number,
        reason,
        executions,
      } => {
        if let Some(current) = self.attempts.last_mut() {
          current.set_status(AttemptStatus::RoutedBack);
        }
        let executions = executions
          .iter()
          .map(|planned| StepExecution::new(planned.id, planned.step.clone(), StepStatus::Pending))
          .collect();
        self.attempts.push(Attempt::new(
          *attempt,
          *number,
          reason.clone(),
          AttemptStatus::Active,
          executions,
        ));
      }
      RunEventKind::CheckpointRecorded {
        checkpoint,
        execution,
        revision,
      } => self.last_checkpoint = Some(Checkpoint::new(*checkpoint, *execution, revision.clone(), at)),
      RunEventKind::UsageRecorded { tokens } => self.used += *tokens,
      RunEventKind::StepStarted { execution } | RunEventKind::StepResumed { execution } => {
        self.set_step(*execution, StepStatus::Running);
      }
      RunEventKind::StepWaiting { execution, reason } => self.set_step(*execution, StepStatus::Waiting(*reason)),
      RunEventKind::PlanDecided { execution, decision } => {
        let status = match decision {
          PlanDecision::Approved => StepStatus::Passed,
          PlanDecision::ChangesRequested => StepStatus::Running,
        };
        self.set_step(*execution, status);
      }
      RunEventKind::StepPassed { execution } => self.set_step(*execution, StepStatus::Passed),
      RunEventKind::StepFailed { execution } => self.set_step(*execution, StepStatus::Failed),
      RunEventKind::StepSkipped { execution } => self.set_step(*execution, StepStatus::Skipped),
    }
  }

  fn set_step(&mut self, execution: StepExecutionId, status: StepStatus) {
    self
      .attempts
      .last_mut()
      .and_then(|attempt| attempt.execution_mut(execution))
      .expect("Run::check found the execution in the current attempt")
      .set_status(status);
  }

  fn end(&mut self, status: RunStatus, attempt: AttemptStatus) {
    self.status = status;
    self.pause_requested = false;
    if let Some(current) = self.attempts.last_mut() {
      current.set_status(attempt);
    }
  }
}

/// Events accepted so far in one transition, applied to a working copy of the run.
pub(super) struct Transition {
  pub(super) run: Run,
  at: Timestamp,
  events: Vec<RunEvent>,
}

impl Transition {
  pub(super) fn record(&mut self, kind: RunEventKind) -> Result<(), RunError> {
    let event = self.run.record(kind, self.at)?;
    self.events.push(event);
    Ok(())
  }

  /// Skips every open execution of the current attempt, so the attempt can end.
  pub(super) fn skip_open(&mut self) -> Result<(), RunError> {
    let open: Vec<StepExecutionId> = self
      .run
      .current_attempt()
      .map(|attempt| {
        attempt
          .executions()
          .iter()
          .filter(|execution| execution.status().is_open())
          .map(StepExecution::id)
          .collect()
      })
      .unwrap_or_default();
    for execution in open {
      self.record(RunEventKind::StepSkipped { execution })?;
    }
    Ok(())
  }
}
