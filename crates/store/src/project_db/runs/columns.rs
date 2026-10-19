//! Status and reason columns of the execution tables. Each enum maps to fixed column texts so
//! the SQL `CHECK` constraints and queries by status stay meaningful.

use orchester_core::{
  resource::StepKey,
  run::{
    AttemptReason, AttemptStatus, InterruptCause, PauseReason, ReviewNote, RunFailure, RunLimit, RunStatus,
    StepFailure, StepStatus, Stuck, WaitReason,
  },
};

use crate::{
  StoreError,
  rows::{corrupt, parse},
};

pub(super) const INTERRUPTED: &str = "interrupted";
pub(super) const RUNNING: &str = "running";

/// `status`, `status_reason`, `failed_step`, `limit_execution` and `failure_reason` of a run row.
pub(super) struct StatusColumns<'a> {
  pub(super) status: &'static str,
  pub(super) reason: Option<&'static str>,
  pub(super) failed_step: Option<&'a str>,
  pub(super) limit_execution: Option<String>,
  /// The reason of a run that failed before it started, as JSON.
  pub(super) failure_reason: Option<String>,
}

pub(super) fn run_status_columns(run_status: &RunStatus) -> StatusColumns<'_> {
  let failure_reason = match run_status {
    RunStatus::Failed(RunFailure::WorktreeNotCreated(reason)) => {
      Some(serde_json::to_string(reason).expect("a worktree failure is plain data with string keys"))
    }
    _ => None,
  };
  let (status, reason, failed_step, limit_execution) = match run_status {
    RunStatus::Pending => ("pending", None, None, None),
    RunStatus::Running => (RUNNING, None, None, None),
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::StepBudget { execution })) => {
      ("paused", Some(STEP_BUDGET), None, Some(execution.to_string()))
    }
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { execution })) => {
      ("paused", Some(PLAN_LIMIT), None, Some(execution.to_string()))
    }
    RunStatus::Paused(reason) => ("paused", Some(pause_reason_text(*reason)), None, None),
    RunStatus::Interrupted(cause) => (INTERRUPTED, Some(interrupt_cause_text(*cause)), None, None),
    RunStatus::WaitingForPerson(Stuck::RetriesExhausted(step)) => {
      (WAITING, Some(RETRIES_EXHAUSTED), Some(step.as_str()), None)
    }
    RunStatus::WaitingForPerson(Stuck::BrokeAgain(step)) => (WAITING, Some(BROKE_AGAIN), Some(step.as_str()), None),
    RunStatus::WaitingForPerson(Stuck::Unverified(step)) => (WAITING, Some(UNVERIFIED), Some(step.as_str()), None),
    RunStatus::WaitingForPerson(Stuck::Unchanged(step)) => (WAITING, Some(UNCHANGED), Some(step.as_str()), None),
    RunStatus::Succeeded => ("succeeded", None, None, None),
    RunStatus::Failed(RunFailure::StepFailed(step)) => ("failed", Some("step_failed"), Some(step.as_str()), None),
    RunStatus::Failed(RunFailure::RetriesExhausted(step)) => {
      ("failed", Some(RETRIES_EXHAUSTED), Some(step.as_str()), None)
    }
    RunStatus::Failed(RunFailure::WorktreeNotCreated(_)) => ("failed", Some(WORKTREE_NOT_CREATED), None, None),
    RunStatus::Cancelled => ("cancelled", None, None, None),
  };
  StatusColumns {
    status,
    reason,
    failed_step,
    limit_execution,
    failure_reason,
  }
}

pub(super) fn parse_run_status(
  status: &str,
  reason: Option<&str>,
  (failed_step, limit_execution): (Option<&str>, Option<&str>),
  failure_reason: Option<&str>,
) -> Result<RunStatus, StoreError> {
  const WHAT: &str = "run status";
  match (status, reason, failure_reason) {
    ("failed", Some(WORKTREE_NOT_CREATED), Some(failure)) if failed_step.is_none() && limit_execution.is_none() => {
      let failure = serde_json::from_str(failure).map_err(|error| corrupt("run failure reason", error))?;
      return Ok(RunStatus::Failed(RunFailure::WorktreeNotCreated(failure)));
    }
    (_, _, Some(failure)) => return Err(corrupt(WHAT, format!("{status} / {reason:?} with reason {failure:?}"))),
    _ => {}
  }
  Ok(match (status, reason, failed_step, limit_execution) {
    ("pending", None, None, None) => RunStatus::Pending,
    (RUNNING, None, None, None) => RunStatus::Running,
    ("paused", Some(STEP_BUDGET), None, Some(execution)) => {
      RunStatus::Paused(PauseReason::LimitReached(RunLimit::StepBudget {
        execution: parse("step execution id", execution)?,
      }))
    }
    ("paused", Some(PLAN_LIMIT), None, Some(execution)) => {
      RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit {
        execution: parse("step execution id", execution)?,
      }))
    }
    ("paused", Some(reason), None, None) => RunStatus::Paused(parse_pause_reason(reason)?),
    (INTERRUPTED, Some(cause), None, None) => RunStatus::Interrupted(parse_interrupt_cause(cause)?),
    (WAITING, Some(RETRIES_EXHAUSTED), Some(step), None) => {
      RunStatus::WaitingForPerson(Stuck::RetriesExhausted(step_key(step)?))
    }
    (WAITING, Some(BROKE_AGAIN), Some(step), None) => RunStatus::WaitingForPerson(Stuck::BrokeAgain(step_key(step)?)),
    (WAITING, Some(UNVERIFIED), Some(step), None) => RunStatus::WaitingForPerson(Stuck::Unverified(step_key(step)?)),
    (WAITING, Some(UNCHANGED), Some(step), None) => RunStatus::WaitingForPerson(Stuck::Unchanged(step_key(step)?)),
    ("succeeded", None, None, None) => RunStatus::Succeeded,
    ("failed", Some("step_failed"), Some(step), None) => RunStatus::Failed(RunFailure::StepFailed(step_key(step)?)),
    ("failed", Some(RETRIES_EXHAUSTED), Some(step), None) => {
      RunStatus::Failed(RunFailure::RetriesExhausted(step_key(step)?))
    }
    ("cancelled", None, None, None) => RunStatus::Cancelled,
    _ => {
      return Err(corrupt(
        WHAT,
        format!("{status} / {reason:?} / {failed_step:?} / {limit_execution:?}"),
      ));
    }
  })
}

/// The pause reason of a run stopped at a step's budget; the execution has a column of its own.
const STEP_BUDGET: &str = "step_budget";
/// The pause reason of a run stopped at its account's plan limit; the execution has a column of its own.
const PLAN_LIMIT: &str = "plan_limit";
/// A run waiting for a person, and why: the step in `failed_step` used up its retries, broke
/// again after its restart, reported unverified, or was not run again on an unchanged revision. A run
/// that failed for used-up retries has the same reason.
const WAITING: &str = "waiting";
const RETRIES_EXHAUSTED: &str = "retries_exhausted";
/// A run that failed before it started, whose worktree could not be created; git's reason has a column of its own.
const WORKTREE_NOT_CREATED: &str = "worktree_not_created";
const BROKE_AGAIN: &str = "broke_again";
const UNVERIFIED: &str = "unverified";
const UNCHANGED: &str = "unchanged";

const fn pause_reason_text(reason: PauseReason) -> &'static str {
  match reason {
    PauseReason::Requested => "requested",
    PauseReason::LimitReached(RunLimit::Tokens) => "token_limit",
    PauseReason::LimitReached(RunLimit::Time) => "time_limit",
    PauseReason::LimitReached(RunLimit::DailyBudget) => "daily_budget",
    PauseReason::LimitReached(RunLimit::GroupBudget) => "group_budget",
    PauseReason::LimitReached(RunLimit::StepBudget { .. }) => STEP_BUDGET,
    PauseReason::LimitReached(RunLimit::PlanLimit { .. }) => PLAN_LIMIT,
  }
}

fn parse_pause_reason(value: &str) -> Result<PauseReason, StoreError> {
  match value {
    "requested" => Ok(PauseReason::Requested),
    "token_limit" => Ok(PauseReason::LimitReached(RunLimit::Tokens)),
    "time_limit" => Ok(PauseReason::LimitReached(RunLimit::Time)),
    "daily_budget" => Ok(PauseReason::LimitReached(RunLimit::DailyBudget)),
    "group_budget" => Ok(PauseReason::LimitReached(RunLimit::GroupBudget)),
    other => Err(corrupt("pause reason", other)),
  }
}

const fn interrupt_cause_text(cause: InterruptCause) -> &'static str {
  match cause {
    InterruptCause::RunnerLost => "runner_lost",
    InterruptCause::QuitNow => "quit_now",
    InterruptCause::Crashed => "crashed",
    InterruptCause::RunnerFailed => "runner_failed",
  }
}

fn parse_interrupt_cause(value: &str) -> Result<InterruptCause, StoreError> {
  match value {
    "runner_lost" => Ok(InterruptCause::RunnerLost),
    "quit_now" => Ok(InterruptCause::QuitNow),
    "crashed" => Ok(InterruptCause::Crashed),
    "runner_failed" => Ok(InterruptCause::RunnerFailed),
    other => Err(corrupt("interrupt cause", other)),
  }
}

/// `reason`, `failed_step`, `return_to`, `note` and `conflicts` of an attempt row.
pub(super) struct AttemptReasonColumns<'a> {
  pub(super) reason: &'a str,
  pub(super) failed_step: Option<&'a str>,
  pub(super) return_to: Option<&'a str>,
  pub(super) note: Option<&'a str>,
  pub(super) conflicts: Option<&'a str>,
}

/// The schema of an attempt's `conflicts`.
pub(super) const CONFLICTS_SCHEMA: u32 = 1;

pub(super) fn attempt_reason_columns(reason: &AttemptReason) -> AttemptReasonColumns<'_> {
  let (text, failed_step, return_to, note) = match reason {
    AttemptReason::Initial => ("initial", None, None, None),
    AttemptReason::Retry { failed_step, return_to } => {
      ("retry", Some(failed_step.as_str()), Some(return_to.as_str()), None)
    }
    AttemptReason::FollowUp { step } => ("follow_up", None, Some(step.as_str()), None),
    AttemptReason::Recovery { step } => ("recovery", None, Some(step.as_str()), None),
    AttemptReason::Restart { step } => ("restart", None, Some(step.as_str()), None),
    AttemptReason::Accepted { step } => ("accepted", None, Some(step.as_str()), None),
    AttemptReason::ReturnedAfterEnd { return_to, note, .. } => (
      "returned_after_end",
      None,
      Some(return_to.as_str()),
      Some(note.as_str()),
    ),
    AttemptReason::Returned {
      failed_step,
      return_to,
      note,
    } => (
      "returned",
      Some(failed_step.as_str()),
      Some(return_to.as_str()),
      Some(note.as_str()),
    ),
  };
  AttemptReasonColumns {
    reason: text,
    failed_step,
    return_to,
    note,
    conflicts: None,
  }
}

/// An attempt's `conflicts` column: what a return after the run ended resolves, as JSON.
pub(super) fn attempt_conflicts_column(reason: &AttemptReason) -> Result<Option<String>, StoreError> {
  match reason {
    AttemptReason::ReturnedAfterEnd {
      conflicts: Some(conflicts),
      ..
    } => crate::rows::encode_json("attempt conflicts", CONFLICTS_SCHEMA, conflicts).map(Some),
    _ => Ok(None),
  }
}

pub(super) fn parse_attempt_reason(columns: &AttemptReasonColumns<'_>) -> Result<AttemptReason, StoreError> {
  let reason = columns.reason;
  match (reason, columns.failed_step, columns.return_to, columns.note) {
    ("initial", None, None, None) => Ok(AttemptReason::Initial),
    ("retry", Some(failed_step), Some(return_to), None) => Ok(AttemptReason::Retry {
      failed_step: step_key(failed_step)?,
      return_to: step_key(return_to)?,
    }),
    ("follow_up", None, Some(step), None) => Ok(AttemptReason::FollowUp { step: step_key(step)? }),
    ("recovery", None, Some(step), None) => Ok(AttemptReason::Recovery { step: step_key(step)? }),
    ("restart", None, Some(step), None) => Ok(AttemptReason::Restart { step: step_key(step)? }),
    ("accepted", None, Some(step), None) => Ok(AttemptReason::Accepted { step: step_key(step)? }),
    ("returned", Some(failed_step), Some(return_to), Some(note)) => Ok(AttemptReason::Returned {
      failed_step: step_key(failed_step)?,
      return_to: step_key(return_to)?,
      note: ReviewNote::new(note).map_err(|error| corrupt("attempt note", error))?,
    }),
    ("returned_after_end", None, Some(return_to), Some(note)) => Ok(AttemptReason::ReturnedAfterEnd {
      return_to: step_key(return_to)?,
      note: ReviewNote::new(note).map_err(|error| corrupt("attempt note", error))?,
      conflicts: columns
        .conflicts
        .map(|text| crate::rows::decode_json("attempt conflicts", CONFLICTS_SCHEMA, text))
        .transpose()?,
    }),
    _ if columns.conflicts.is_some() => Err(corrupt("attempt conflicts", reason)),
    _ => Err(corrupt("attempt reason", reason)),
  }
}

pub(super) const fn attempt_status_text(status: AttemptStatus) -> &'static str {
  match status {
    AttemptStatus::Active => "active",
    AttemptStatus::RoutedBack => "routed_back",
    AttemptStatus::Succeeded => "succeeded",
    AttemptStatus::Failed => "failed",
    AttemptStatus::Cancelled => "cancelled",
  }
}

pub(super) fn parse_attempt_status(value: &str) -> Result<AttemptStatus, StoreError> {
  match value {
    "active" => Ok(AttemptStatus::Active),
    "routed_back" => Ok(AttemptStatus::RoutedBack),
    "succeeded" => Ok(AttemptStatus::Succeeded),
    "failed" => Ok(AttemptStatus::Failed),
    "cancelled" => Ok(AttemptStatus::Cancelled),
    other => Err(corrupt("attempt status", other)),
  }
}

/// `status` and `wait_reason` of a step execution row.
pub(super) const fn step_status_columns(status: StepStatus) -> (&'static str, Option<&'static str>) {
  match status {
    StepStatus::Pending => ("pending", None),
    StepStatus::Running => ("running", None),
    StepStatus::Waiting(WaitReason::Approval) => ("waiting", Some("approval")),
    StepStatus::Waiting(WaitReason::Plan) => ("waiting", Some("plan")),
    StepStatus::Waiting(WaitReason::Message) => ("waiting", Some("message")),
    StepStatus::Passed => ("passed", None),
    StepStatus::Failed => ("failed", None),
    StepStatus::Skipped => ("skipped", None),
  }
}

pub(super) fn parse_step_status(status: &str, wait_reason: Option<&str>) -> Result<StepStatus, StoreError> {
  Ok(match (status, wait_reason) {
    ("pending", None) => StepStatus::Pending,
    ("running", None) => StepStatus::Running,
    ("waiting", Some("approval")) => StepStatus::Waiting(WaitReason::Approval),
    ("waiting", Some("plan")) => StepStatus::Waiting(WaitReason::Plan),
    ("waiting", Some("message")) => StepStatus::Waiting(WaitReason::Message),
    ("passed", None) => StepStatus::Passed,
    ("failed", None) => StepStatus::Failed,
    ("skipped", None) => StepStatus::Skipped,
    _ => return Err(corrupt("step status", format!("{status} / {wait_reason:?}"))),
  })
}

/// `failure` of a failed step execution row.
pub(super) const fn step_failure_text(failure: StepFailure) -> &'static str {
  match failure {
    StepFailure::AgentFailed => "agent_failed",
    StepFailure::ReportedFailed => "reported_failed",
    StepFailure::NotStarted => "not_started",
    // Legacy: no longer recorded, still read.
    StepFailure::StepBudget => "step_budget",
    StepFailure::MergeConflict => "merge_conflict",
    StepFailure::CheckpointFailed => "checkpoint_failed",
    StepFailure::CommandFailed => "command_failed",
    StepFailure::Unverified => UNVERIFIED,
    StepFailure::Unchanged => UNCHANGED,
  }
}

pub(super) fn parse_step_failure(value: &str) -> Result<StepFailure, StoreError> {
  Ok(match value {
    "agent_failed" => StepFailure::AgentFailed,
    "reported_failed" => StepFailure::ReportedFailed,
    "not_started" => StepFailure::NotStarted,
    "step_budget" => StepFailure::StepBudget,
    "merge_conflict" => StepFailure::MergeConflict,
    "checkpoint_failed" => StepFailure::CheckpointFailed,
    "command_failed" => StepFailure::CommandFailed,
    UNVERIFIED => StepFailure::Unverified,
    UNCHANGED => StepFailure::Unchanged,
    other => return Err(corrupt("step failure", other)),
  })
}

pub(super) fn step_key(value: &str) -> Result<StepKey, StoreError> {
  StepKey::new(value).map_err(|error| corrupt("step key", error))
}
