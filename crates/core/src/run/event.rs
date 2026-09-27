use std::{
  num::{NonZeroU32, NonZeroU64},
  time::Duration,
};

use serde::{Deserialize, Serialize};

use super::{
  branch::BranchName,
  number::RunNumber,
  permissions::{PermissionsApply, PermissionsChangedBy},
  plan::{PlanDecision, PlanVersion},
  snapshot::RunSnapshot,
  status::{AttemptReason, ContinueTrigger, InterruptCause, PauseReason, RunFailure, StepFailure, WaitReason},
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
  pub budget: TokenBudget,
  /// The most active agent time the run may use; pauses and waits for a person do not count.
  #[serde(with = "millis")]
  pub max_duration: Duration,
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
  Started,
  /// A person asked to pause; the run keeps working until its active step reaches a checkpoint.
  PauseRequested,
  /// The run settled at a checkpoint and stopped scheduling.
  Paused {
    reason: PauseReason,
  },
  Resumed,
  /// A run stopped at a limit got new limits; it continues with a [`RunEventKind::Resumed`].
  LimitRaised {
    budget: TokenBudget,
    #[serde(with = "millis")]
    max_duration: Duration,
  },
  /// A run stopped at the budget of `step` may use `budget` tokens in each execution of that step
  /// from now on; it continues with a [`RunEventKind::Resumed`].
  StepBudgetRaised {
    step: StepKey,
    budget: TokenBudget,
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
  /// The follow-up attempt's execution failed: the attempt fails, and the run is succeeded again
  /// with the result it had.
  FollowUpFailed,
  Failed {
    reason: RunFailure,
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
  },
  StepPassed {
    execution: StepExecutionId,
  },
  StepFailed {
    execution: StepExecutionId,
    /// Logs written before failures had reasons have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<StepFailure>,
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
      | Self::StepSkipped { execution }
      | Self::CheckpointRecorded { execution, .. }
      | Self::PermissionsApplied { execution, .. } => Some(*execution),
      _ => None,
    }
  }

  /// Stable name of the kind, as stored and shown in errors.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Created(_) => "created",
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
      Self::FollowUpFailed => "follow_up_failed",
      Self::Failed { .. } => "failed",
      Self::Cancelled => "cancelled",
      Self::AttemptStarted { .. } => "attempt_started",
      Self::StepStarted { .. } => "step_started",
      Self::StepWaiting { .. } => "step_waiting",
      Self::StepResumed { .. } => "step_resumed",
      Self::PlanDecided { .. } => "plan_decided",
      Self::StepPassed { .. } => "step_passed",
      Self::StepFailed { .. } => "step_failed",
      Self::StepSkipped { .. } => "step_skipped",
      Self::CheckpointRecorded { .. } => "checkpoint_recorded",
      Self::UsageRecorded { .. } => "usage_recorded",
    }
  }
}

/// A duration stored as whole milliseconds.
mod millis {
  use std::time::Duration;

  use serde::{Deserialize, Deserializer, Serializer};

  pub(super) fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    let millis = u64::try_from(duration.as_millis()).map_err(serde::ser::Error::custom)?;
    serializer.serialize_u64(millis)
  }

  pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    u64::deserialize(deserializer).map(Duration::from_millis)
  }
}
