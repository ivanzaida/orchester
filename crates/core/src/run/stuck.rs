//! What happens after a step fails, and what a person decides when no automatic route is left (user
//! decisions 2026-10-02). A verdict on the work (the agent reported `failed`, the command's exit code
//! said so) follows the step's failure route; once its retries are used up the run waits for a person.
//! Any other failure is a breakage: the step restarts once as a fresh execution, and breaking again
//! in that restart makes the run wait for a person too. Breakages never return to an upstream step.
//!
//! A run waiting for a person goes on when they allow one more retry (the same route, or the same
//! restart), return the work to an agent step with a note, or stop it, which fails it as before.
//! A retry attempt's prompt reports the failure that sent the work back ([`Run::retry_report`]).

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use super::{
  attempt::StepExecution,
  error::RunError,
  event::{PlannedExecution, RunEvent, RunEventKind},
  plan::ReviewNote,
  snapshot::StepExecutor,
  state::Run,
  status::{AttemptReason, RunFailure, RunStatus, StepFailure, StepStatus, Stuck},
};
use crate::{
  id::{AttemptId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

/// A person's decision on a run that waits for them ([`RunStatus::WaitingForPerson`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StuckDecision {
  /// One more attempt: the step's failure route again, or for a step that broke, waited unverified or was
  /// not run again on unchanged work, the step again.
  Retry,
  /// For a step that waited unverified or unchanged: it passes as it is, and the steps after it and those that
  /// have not passed run ([`AttemptReason::Accepted`]). `by`: a person, or automation a person let answer approvals.
  Accept { by: super::answer::Answerer },
  /// A new attempt from agent step `step` (one of [`Run::return_targets`]), whose prompt carries
  /// `note` as the person's instruction.
  ReturnTo { step: StepKey, note: ReviewNote },
  /// The run fails, as it did before runs waited for a person.
  Stop,
}

/// The failure a retry attempt's execution is told about: the execution that failed and sent the
/// work back, the attempt it failed in, and the person's note when one returned the work to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryReportSource<'a> {
  pub failed: &'a StepExecution,
  pub attempt: NonZeroU32,
  pub note: Option<&'a ReviewNote>,
}

/// The most automatic attempts — retries of a failure route and restarts after a breakage — a run
/// starts since its last attempt that was not automatic (the first, or one a person started by
/// returning the work or recovering the run). Any failure after them waits for a person, so steps
/// that fail in turn never retry each other without end.
pub const MAX_AUTOMATIC_ATTEMPTS: usize = 10;

impl Run {
  /// What follows the failure of `step` for `reason` in an attempt that is not a follow-up.
  pub(super) fn after_failure(&self, step: &StepKey, reason: StepFailure) -> RunEventKind {
    match reason {
      StepFailure::Unverified => {
        return RunEventKind::WaitingForPerson {
          reason: Stuck::Unverified(step.clone()),
        };
      }
      StepFailure::Unchanged => {
        return RunEventKind::WaitingForPerson {
          reason: Stuck::Unchanged(step.clone()),
        };
      }
      _ => {}
    }
    if !reason.is_verdict() {
      // The failure just recorded counts: a second breakage since the step last passed waits.
      return if self.breakages_since_passed(step) >= 2 || self.automatic_attempts() >= MAX_AUTOMATIC_ATTEMPTS {
        RunEventKind::WaitingForPerson {
          reason: Stuck::BrokeAgain(step.clone()),
        }
      } else {
        self.attempt(AttemptReason::Restart { step: step.clone() }, step)
      };
    }
    let route = self
      .snapshot()
      .body()
      .step(step)
      .expect("executions name snapshot steps")
      .on_failure
      .clone();
    match route {
      None => RunEventKind::Failed {
        reason: RunFailure::StepFailed(step.clone()),
      },
      Some(route)
        if self.retries_taken(step) >= route.max_retries.get()
          || self.automatic_attempts() >= MAX_AUTOMATIC_ATTEMPTS =>
      {
        RunEventKind::WaitingForPerson {
          reason: Stuck::RetriesExhausted(step.clone()),
        }
      }
      Some(route) => self.attempt(
        AttemptReason::Retry {
          failed_step: step.clone(),
          return_to: route.return_to.clone(),
        },
        &route.return_to,
      ),
    }
  }

  /// What follows the failure of `step` for `reason` in a follow-up of a run that had succeeded (the project task
  /// ORC-44): a verdict on the work routes the run as it would in any attempt — the step's failure route while it has
  /// retries left, else a wait for a person — and where the pipeline has no route, the run waits for a person, who
  /// returns the work or stops the run, instead of failing or staying succeeded. `None` for any other failure, which
  /// leaves the run as it was.
  pub(super) fn after_follow_up_failure(&self, step: &StepKey, reason: StepFailure) -> Option<RunEventKind> {
    if !reason.is_verdict() {
      return None;
    }
    Some(match self.after_failure(step, reason) {
      RunEventKind::Failed { .. } => RunEventKind::WaitingForPerson {
        reason: Stuck::RetriesExhausted(step.clone()),
      },
      next => next,
    })
  }

  /// How often `step` broke — failed without a verdict on its work — since it last passed, across
  /// attempts.
  fn breakages_since_passed(&self, step: &StepKey) -> usize {
    self
      .attempts()
      .iter()
      .filter(|attempt| attempt.counts())
      .flat_map(|attempt| attempt.executions())
      .filter(|execution| execution.step() == step)
      .fold(0, |broken, execution| match execution.status() {
        StepStatus::Passed => 0,
        StepStatus::Failed if execution.failure().is_none_or(StepFailure::is_breakage) => broken + 1,
        _ => broken,
      })
  }

  /// Automatic attempts since the run's last attempt that was not automatic.
  fn automatic_attempts(&self) -> usize {
    self
      .attempts()
      .iter()
      .rev()
      .take_while(|attempt| {
        matches!(
          attempt.reason(),
          AttemptReason::Retry { .. } | AttemptReason::Restart { .. }
        )
      })
      .count()
  }

  /// How often the failure route of `step` was taken by itself: its retries up to the route's
  /// `max_retries`. Retries beyond them were ones a person allowed, which "still fails after N
  /// retries" does not count.
  pub fn automatic_retries_taken(&self, step: &StepKey) -> u32 {
    let allowed = self
      .snapshot()
      .body()
      .step(step)
      .and_then(|pinned| pinned.on_failure.as_ref())
      .map_or(0, |route| route.max_retries.get());
    self.retries_taken(step).min(allowed)
  }

  /// Whether the run waits because a follow-up's step reported the work failed where the pipeline has no route for
  /// that failure (the project task ORC-44). It waits as after used-up retries, but took none: one more
  /// retry runs the step itself again, and people are told the step failed, not that its retries ran out.
  pub fn waits_without_route(&self) -> bool {
    let RunStatus::WaitingForPerson(Stuck::RetriesExhausted(step)) = self.status() else {
      return false;
    };
    self
      .snapshot()
      .body()
      .step(step)
      .is_some_and(|pinned| pinned.on_failure.is_none())
  }

  /// The step one more retry of a waiting run starts from: the failure route's target after used-up
  /// retries, the step itself after it broke again, or after a follow-up of a step without a route failed.
  /// `None` for a run that does not wait.
  pub fn retry_from(&self) -> Option<&StepKey> {
    let RunStatus::WaitingForPerson(stuck) = self.status() else {
      return None;
    };
    Some(match stuck {
      Stuck::RetriesExhausted(step) => self
        .snapshot()
        .body()
        .step(step)
        .and_then(|pinned| pinned.on_failure.as_ref())
        .map_or(step, |route| &route.return_to),
      Stuck::BrokeAgain(step) | Stuck::Unverified(step) | Stuck::Unchanged(step) => step,
    })
  }

  /// A new attempt for `reason` that executes `from` again and everything after it.
  fn attempt(&self, reason: AttemptReason, from: &StepKey) -> RunEventKind {
    RunEventKind::AttemptStarted {
      attempt: AttemptId::generate(),
      number: self.next_attempt_number(),
      executions: self.retry_executions(from),
      reason,
    }
  }

  /// The agent steps a person may return the work of a waiting run to, in pipeline order: the step
  /// that failed and every agent step it runs after, each of which has executed. Empty for a run
  /// that does not wait for a person.
  pub fn return_targets(&self) -> Vec<StepKey> {
    let RunStatus::WaitingForPerson(stuck) = self.status() else {
      return Vec::new();
    };
    self
      .snapshot()
      .steps()
      .iter()
      .filter(|step| matches!(step.executor(), StepExecutor::Agent { .. }))
      .map(|step| step.key())
      .filter(|key| self.snapshot().downstream_of(key).contains(stuck.step()))
      .filter(|key| self.latest_execution(key).is_some())
      .cloned()
      .collect()
  }

  /// The return target ([`Run::return_targets`]) a person's choice starts at. After used-up retries, the failure
  /// route's target; after a breakage, the step itself. A step that waited unverified or was not run again is run
  /// again by *Retry* already, so the choice starts elsewhere: at the step this attempt returned the work to (after
  /// a review sent it back and nothing changed), else the step's failure route target, else the nearest agent step
  /// before it; at the step itself only when no other is left. `None` for a run that does not wait or has no target.
  pub fn return_default(&self) -> Option<StepKey> {
    let RunStatus::WaitingForPerson(stuck) = self.status() else {
      return None;
    };
    let targets = self.return_targets();
    let step = stuck.step();
    let preferred = match stuck {
      Stuck::RetriesExhausted(_) | Stuck::BrokeAgain(_) => self.retry_from(),
      Stuck::Unverified(_) | Stuck::Unchanged(_) => {
        let returned = self.current_attempt().and_then(|attempt| match attempt.reason() {
          AttemptReason::Retry { return_to, .. } | AttemptReason::Returned { return_to, .. } => Some(return_to),
          _ => None,
        });
        let route = self
          .snapshot()
          .body()
          .step(step)
          .and_then(|pinned| pinned.on_failure.as_ref())
          .map(|route| &route.return_to);
        [returned, route]
          .into_iter()
          .flatten()
          .find(|key| *key != step && targets.contains(key))
          .or_else(|| targets.iter().rev().find(|key| *key != step))
      }
    };
    preferred
      .filter(|key| targets.contains(key))
      .or_else(|| targets.first())
      .cloned()
  }

  /// Carries out a person's `decision` on a run that waits for them.
  pub fn decide_stuck(&mut self, decision: &StuckDecision, now: Timestamp) -> Result<Vec<RunEvent>, RunError> {
    let by = match decision {
      StuckDecision::Accept { by } => *by,
      _ => super::Answerer::Person,
    };
    self.decide_stuck_as(decision, by, now)
  }

  /// Carries out and records a decision with the identity captured when input was delivered.
  pub fn decide_stuck_as(
    &mut self,
    decision: &StuckDecision,
    by: super::Answerer,
    now: Timestamp,
  ) -> Result<Vec<RunEvent>, RunError> {
    let RunStatus::WaitingForPerson(stuck) = self.status().clone() else {
      return Err(RunError::InvalidRunTransition {
        status: self.status().clone(),
        event: RunEventKind::RetryAllowed.name(),
      });
    };
    let step = stuck.step().clone();
    let (reason, from) = match decision {
      StuckDecision::Stop => {
        return self.transition(now, |transition| {
          transition.record(RunEventKind::StuckDecided {
            decision: decision.clone(),
            by,
          })?;
          transition.record(RunEventKind::Failed {
            reason: stuck.failure(),
          })
        });
      }
      StuckDecision::Accept { .. } => return self.accept_stuck(&stuck, by, now),
      StuckDecision::Retry => match &stuck {
        Stuck::BrokeAgain(_) | Stuck::Unverified(_) | Stuck::Unchanged(_) => {
          (AttemptReason::Restart { step: step.clone() }, step)
        }
        Stuck::RetriesExhausted(_) => {
          let return_to = self.retry_from().unwrap_or(&step).clone();
          let reason = AttemptReason::Retry {
            failed_step: step,
            return_to: return_to.clone(),
          };
          (reason, return_to)
        }
      },
      StuckDecision::ReturnTo { step: target, note } => {
        if !self.return_targets().contains(target) {
          return Err(RunError::NotAReturnTarget(target.clone()));
        }
        let reason = AttemptReason::Returned {
          failed_step: step,
          return_to: target.clone(),
          note: note.clone(),
        };
        (reason, target.clone())
      }
    };
    self.transition(now, |transition| {
      transition.record(RunEventKind::StuckDecided {
        decision: decision.clone(),
        by,
      })?;
      transition.record(RunEventKind::RetryAllowed)?;
      let attempt = transition.run.attempt(reason, &from);
      transition.record(attempt)
    })
  }

  /// A run fails from work (its last step failed) or when a person stops it while it waits for them,
  /// for the reason it waits.
  pub(super) fn check_failed(&self, kind: &RunEventKind, reason: &RunFailure) -> Result<(), RunError> {
    let not_started = matches!(reason, RunFailure::WorktreeNotCreated(_));
    match self.status() {
      RunStatus::Pending => self.expect_status(kind, not_started),
      RunStatus::WaitingForPerson(stuck) => self.expect_status(kind, stuck.failure() == *reason),
      status => {
        self.expect_status(kind, *status == RunStatus::Running && !not_started)?;
        self.expect_settled()
      }
    }
  }

  /// A restart executes a step whose latest execution broke, and everything after it.
  pub(super) fn check_restart(&self, step: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    let broke = self.latest_execution(step).is_some_and(|execution| {
      execution.status() == StepStatus::Failed && !execution.failure().is_some_and(StepFailure::is_verdict)
    });
    // `Restart` also runs a step again that waited unverified or unchanged, when a person asks for it.
    if !broke {
      return Err(RunError::InvalidAttempt("a restart executes a step that broke"));
    }
    self.expect_retry_executions(step, executions)
  }

  /// Work a person returned executes an agent step and everything after it.
  pub(super) fn check_returned(&self, return_to: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    if !matches!(
      self.snapshot().step(return_to).map(|step| step.executor()),
      Some(StepExecutor::Agent { .. })
    ) {
      return Err(RunError::InvalidAttempt("work is returned to an agent step"));
    }
    self.expect_retry_executions(return_to, executions)
  }

  fn expect_retry_executions(&self, from: &StepKey, executions: &[PlannedExecution]) -> Result<(), RunError> {
    let expected = self.retry_executions(from);
    if executions
      .iter()
      .map(|planned| &planned.step)
      .eq(expected.iter().map(|planned| &planned.step))
    {
      Ok(())
    } else {
      Err(RunError::InvalidAttempt(
        "the attempt executes the step it returns to, its dependent and its unfinished steps",
      ))
    }
  }

  /// The failure the prompt of `execution` reports, when it belongs to an attempt that a failure
  /// sent back: the step the work returned to learns why, and the step that failed gets its own
  /// earlier result to check against. A restart after a breakage reports what the attempt it
  /// restarts reported. Any other execution reports none.
  pub fn retry_report(&self, execution: StepExecutionId) -> Option<RetryReportSource<'_>> {
    let position = self
      .attempts()
      .iter()
      .position(|attempt| attempt.execution(execution).is_some())?;
    let step = self.attempts()[position].execution(execution)?.step();
    let origin = self.attempts()[..=position]
      .iter()
      .rposition(|attempt| !matches!(attempt.reason(), AttemptReason::Restart { .. }))?;
    let (failed_step, return_to, note) = match self.attempts()[origin].reason() {
      AttemptReason::Retry { failed_step, return_to } => (failed_step, return_to, None),
      AttemptReason::Returned {
        failed_step,
        return_to,
        note,
      } => (failed_step, return_to, Some(note)),
      _ => return None,
    };
    if step != failed_step && step != return_to {
      return None;
    }
    let (earlier, failed) = self.attempts()[..origin].iter().rev().find_map(|earlier| {
      earlier
        .executions()
        .iter()
        .find(|candidate| candidate.step() == failed_step && candidate.status() == StepStatus::Failed)
        .map(|failed| (earlier, failed))
    })?;
    Some(RetryReportSource {
      failed,
      attempt: earlier.number(),
      note: note.filter(|_| step == return_to),
    })
  }
}

/// When an attempt started, and whether a person's decision started it (a retry or return they
/// chose for a run that waited for them, or the work of an ended run they returned) rather than the run by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptStart {
  pub attempt: AttemptId,
  pub at: Timestamp,
  pub by_person: bool,
  pub by: Option<super::Answerer>,
}

fn attempt_decider(events: &[RunEvent]) -> Option<super::Answerer> {
  events
    .iter()
    .rev()
    .take_while(|event| !matches!(event.kind(), RunEventKind::AttemptStarted { .. }))
    .find_map(|event| match event.kind() {
      RunEventKind::StuckDecided { by, .. }
      | RunEventKind::RecoveryDecided { by, .. }
      | RunEventKind::ReturnDecided { by, .. }
      | RunEventKind::StepAccepted { by, .. } => Some(*by),
      _ => None,
    })
    .or_else(|| {
      events
        .last()
        .and_then(|event| matches!(event.kind(), RunEventKind::RetryAllowed).then_some(super::Answerer::Person))
    })
}

/// The start of every attempt in `events`, a run's log in sequence order.
pub fn attempt_starts(events: &[RunEvent]) -> Vec<AttemptStart> {
  events
    .iter()
    .enumerate()
    .filter_map(|(index, event)| match event.kind() {
      RunEventKind::AttemptStarted { attempt, .. } => Some(AttemptStart {
        attempt: *attempt,
        at: event.at(),
        by_person: attempt_decider(&events[..index]) == Some(super::Answerer::Person),
        by: attempt_decider(&events[..index]),
      }),
      _ => None,
    })
    .collect()
}
