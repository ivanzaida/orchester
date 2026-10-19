use std::{
  num::{NonZeroU32, NonZeroU64},
  time::Duration,
};

use serde::{Deserialize, Serialize};

use super::{
  answer::Answerer,
  branch::BranchName,
  number::RunNumber,
  permissions::{PermissionsApply, PermissionsChangedBy},
  plan::{PlanDecision, PlanVersion},
  snapshot::RunSnapshot,
  status::{AttemptReason, ContinueTrigger, InterruptCause, PauseReason, RunFailure, StepFailure, Stuck, WaitReason},
};
use crate::{
  id::{AttemptId, CheckpointId, ResourceId, RunId, StepExecutionId, TaskId},
  permission::AgentPermissions,
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};

/// Position of an event in its run's log, starting at 1 without gaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventSequence(NonZeroU64);

impl EventSequence {
  pub const FIRST: Self = Self(NonZeroU64::MIN);

  pub const fn new(sequence: NonZeroU64) -> Self {
    Self(sequence)
  }

  pub const fn get(self) -> u64 {
    self.0.get()
  }

  pub fn next(self) -> Self {
    Self(self.0.checked_add(1).expect("a run log never reaches 2^64 events"))
  }
}

/// One append-only entry of a run's log. A run's current state is the result of applying its
/// events in sequence order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEvent {
  run: RunId,
  sequence: EventSequence,
  at: Timestamp,
  kind: RunEventKind,
}

impl RunEvent {
  pub const fn new(run: RunId, sequence: EventSequence, at: Timestamp, kind: RunEventKind) -> Self {
    Self {
      run,
      sequence,
      at,
      kind,
    }
  }

  pub const fn run(&self) -> RunId {
    self.run
  }

  pub const fn sequence(&self) -> EventSequence {
    self.sequence
  }

  pub const fn at(&self) -> Timestamp {
    self.at
  }

  pub const fn kind(&self) -> &RunEventKind {
    &self.kind
  }
}

/// Creation facts of a run; the first event of every log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunCreated {
  pub task: TaskId,
  pub number: RunNumber,
  pub snapshot: RunSnapshot,
  pub branch: BranchName,
  pub base: Revision,
  /// The branch the user's checkout had when the run started, which merging the run brings its
  /// work into; `None` when the checkout had a detached HEAD, and for runs recorded before
  /// Orchester recorded it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub base_branch: Option<BranchName>,
  /// `None`: the run has no token budget.
  #[serde(default)]
  pub budget: Option<TokenBudget>,
  /// The most active agent time the run may use; pauses and waits for a person do not count.
  /// `None`: the run has no time limit.
  #[serde(default, with = "optional_millis")]
  pub max_duration: Option<Duration>,
}

/// A step execution created by a new attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedExecution {
  pub id: StepExecutionId,
  pub step: StepKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RunEventKind {
  Created(Box<RunCreated>),
  /// A branch of the pending run's name existed in its repository when its worktree was made, so
  /// the run works on `branch`, a free name, instead ([`super::Run::choose_branch`]).
  BranchChosen {
    branch: BranchName,
  },
  Started,
  /// A person asked to pause; the run keeps working until its active step reaches a checkpoint.
  PauseRequested,
  /// The run settled at a checkpoint and stopped scheduling.
  Paused {
    reason: PauseReason,
  },
  Resumed,
  /// A run stopped at a limit got new limits; it continues with a [`RunEventKind::Resumed`].
  /// `None` lifts that limit.
  LimitRaised {
    #[serde(default)]
    budget: Option<TokenBudget>,
    #[serde(default, with = "optional_millis")]
    max_duration: Option<Duration>,
  },
  /// A run stopped at the budget of `step` may use `budget` tokens in each execution of that step
  /// from now on, or any number with `None`; it continues with a [`RunEventKind::Resumed`].
  StepBudgetRaised {
    step: StepKey,
    #[serde(default)]
    budget: Option<TokenBudget>,
  },
  Interrupted {
    cause: InterruptCause,
  },
  /// The run uses `permissions` for agent profile `profile` from now on (02.J); with
  /// [`PermissionsApply::Now`] its agents at work restart at their next checkpoint.
  PermissionsChanged {
    profile: ResourceId,
    permissions: AgentPermissions,
    apply: PermissionsApply,
    by: PermissionsChangedBy,
  },
  /// The pending restarts of `profile`'s agents are cancelled; the change applies from their next
  /// invocations.
  PermissionsRestartCancelled {
    profile: ResourceId,
  },
  /// `execution`'s agent ended at `checkpoint` and continues with the changed permissions.
  PermissionsApplied {
    execution: StepExecutionId,
    checkpoint: Option<CheckpointId>,
  },
  /// Continues an interrupted run from `from`, its last checkpoint (`None` if it had none).
  Continued {
    trigger: ContinueTrigger,
    from: Option<CheckpointId>,
  },
  Succeeded,
  /// A succeeded run works again: a follow-up attempt comes with this event
  /// ([`super::Run::follow_up`]).
  Reopened,
  /// A person resumes a failed run through its conversation; a recovery attempt follows.
  RecoveryRequested,
  /// The follow-up attempt's execution failed: the attempt fails, and the run is succeeded again
  /// with the result it had.
  FollowUpFailed,
  Failed {
    reason: RunFailure,
  },
  /// The attempt ended for `reason` with no automatic route left: the run waits for a person, who
  /// allows another attempt ([`RunEventKind::RetryAllowed`]) or stops it ([`RunEventKind::Failed`]).
  WaitingForPerson {
    reason: Stuck,
  },
  /// A person let a run that waited for them go on; the attempt they chose follows.
  RetryAllowed,
  /// Who chose how a waiting run proceeds, including retries, returns, acceptance and stopping.
  StuckDecided {
    decision: super::StuckDecision,
    by: Answerer,
  },
  /// A message explicitly continued a failed or waiting run from this step.
  RecoveryDecided {
    step: StepKey,
    by: Answerer,
  },
  Cancelled,
  /// Opens attempt `number` with fresh executions for `executions`; steps that passed in an
  /// earlier attempt and are not listed keep that result.
  AttemptStarted {
    attempt: AttemptId,
    number: NonZeroU32,
    reason: AttemptReason,
    executions: Vec<PlannedExecution>,
  },
  StepStarted {
    execution: StepExecutionId,
  },
  StepWaiting {
    execution: StepExecutionId,
    reason: WaitReason,
  },
  /// A step waiting for an approval or a message works again.
  StepResumed {
    execution: StepExecutionId,
  },
  /// A person decided on plan `version` of the gated step `execution` waits with.
  PlanDecided {
    execution: StepExecutionId,
    version: PlanVersion,
    decision: PlanDecision,
    #[serde(default)]
    by: Answerer,
  },
  StepPassed {
    execution: StepExecutionId,
  },
  StepFailed {
    execution: StepExecutionId,
    /// Logs written before failures had reasons have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<StepFailure>,
    /// The revision of the run's branch the execution started on, which its result was for; `None` when
    /// the runner did not know it (the agent never started) and in logs written before it was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revision: Option<Revision>,
  },
  /// `by` accepted `execution`, which waited for a person unverified or unchanged, as it is
  /// ([`super::StuckDecision::Accept`]); the attempt that follows passes its step.
  StepAccepted {
    execution: StepExecutionId,
    by: Answerer,
  },
  StepSkipped {
    execution: StepExecutionId,
  },
  CheckpointRecorded {
    checkpoint: CheckpointId,
    execution: StepExecutionId,
    revision: Revision,
  },
  /// Tokens an agent CLI reported, as far as they count toward the budget.
  UsageRecorded {
    tokens: TokensUsed,
  },
}

impl RunEventKind {
  /// The step execution a step-level event changes; `None` for run-level events.
  pub const fn execution(&self) -> Option<StepExecutionId> {
    match self {
      Self::StepStarted { execution }
      | Self::StepWaiting { execution, .. }
      | Self::StepResumed { execution }
      | Self::PlanDecided { execution, .. }
      | Self::StepPassed { execution }
      | Self::StepFailed { execution, .. }
      | Self::StepAccepted { execution, .. }
      | Self::StepSkipped { execution }
      | Self::CheckpointRecorded { execution, .. }
      | Self::PermissionsApplied { execution, .. } => Some(*execution),
      _ => None,
    }
  }

  /// Starts or resumes execution, including follow-ups, a retry a person allows and failed-run
  /// recovery.
  pub const fn starts_execution(&self) -> bool {
    matches!(
      self,
      Self::Started
        | Self::Resumed
        | Self::Continued { .. }
        | Self::Reopened
        | Self::RecoveryRequested
        | Self::RetryAllowed
    )
  }

  /// Stable name of the kind, as stored and shown in errors.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Created(_) => "created",
      Self::BranchChosen { .. } => "branch_chosen",
      Self::Started => "started",
      Self::PauseRequested => "pause_requested",
      Self::Paused { .. } => "paused",
      Self::Resumed => "resumed",
      Self::LimitRaised { .. } => "limit_raised",
      Self::StepBudgetRaised { .. } => "step_budget_raised",
      Self::Interrupted { .. } => "interrupted",
      Self::PermissionsChanged { .. } => "permissions_changed",
      Self::PermissionsRestartCancelled { .. } => "permissions_restart_cancelled",
      Self::PermissionsApplied { .. } => "permissions_applied",
      Self::Continued { .. } => "continued",
      Self::Succeeded => "succeeded",
      Self::Reopened => "reopened",
      Self::RecoveryRequested => "recovery_requested",
      Self::FollowUpFailed => "follow_up_failed",
      Self::Failed { .. } => "failed",
      Self::WaitingForPerson { .. } => "waiting_for_person",
      Self::RetryAllowed => "retry_allowed",
      Self::StuckDecided { .. } => "stuck_decided",
      Self::RecoveryDecided { .. } => "recovery_decided",
      Self::Cancelled => "cancelled",
      Self::AttemptStarted { .. } => "attempt_started",
      Self::StepStarted { .. } => "step_started",
      Self::StepWaiting { .. } => "step_waiting",
      Self::StepResumed { .. } => "step_resumed",
      Self::PlanDecided { .. } => "plan_decided",
      Self::StepPassed { .. } => "step_passed",
      Self::StepFailed { .. } => "step_failed",
      Self::StepAccepted { .. } => "step_accepted",
      Self::StepSkipped { .. } => "step_skipped",
      Self::CheckpointRecorded { .. } => "checkpoint_recorded",
      Self::UsageRecorded { .. } => "usage_recorded",
    }
  }
}

/// An optional duration stored as whole milliseconds, `null` for none.
mod optional_millis {
  use std::time::Duration;

  use serde::{Deserialize, Deserializer, Serializer};

  pub(super) fn serialize<S: Serializer>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error> {
    match duration {
      Some(duration) => {
        let millis = u64::try_from(duration.as_millis()).map_err(serde::ser::Error::custom)?;
        serializer.serialize_some(&millis)
      }
      None => serializer.serialize_none(),
    }
  }

  pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Duration>, D::Error> {
    Option::<u64>::deserialize(deserializer).map(|millis| millis.map(Duration::from_millis))
  }
}
