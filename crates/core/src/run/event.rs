use std::num::{NonZeroU32, NonZeroU64};

use serde::{Deserialize, Serialize};

use super::{
  branch::BranchName,
  number::RunNumber,
  snapshot::RunSnapshot,
  status::{AttemptReason, ContinueTrigger, InterruptCause, PlanDecision, RunFailure, WaitReason},
};
use crate::{
  id::{AttemptId, CheckpointId, RunId, StepExecutionId, TaskId},
  money::Money,
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
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
  pub budget: Money,
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
  Paused,
  Resumed,
  Interrupted {
    cause: InterruptCause,
  },
  /// Continues an interrupted run from `from`, its last checkpoint (`None` if it had none).
  Continued {
    trigger: ContinueTrigger,
    from: Option<CheckpointId>,
  },
  Succeeded,
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
  PlanDecided {
    execution: StepExecutionId,
    decision: PlanDecision,
  },
  StepPassed {
    execution: StepExecutionId,
  },
  StepFailed {
    execution: StepExecutionId,
  },
  StepSkipped {
    execution: StepExecutionId,
  },
  CheckpointRecorded {
    checkpoint: CheckpointId,
    execution: StepExecutionId,
    revision: Revision,
  },
  SpendRecorded {
    amount: Money,
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
      | Self::StepFailed { execution }
      | Self::StepSkipped { execution }
      | Self::CheckpointRecorded { execution, .. } => Some(*execution),
      _ => None,
    }
  }

  /// Stable name of the kind, as stored and shown in errors.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Created(_) => "created",
      Self::Started => "started",
      Self::Paused => "paused",
      Self::Resumed => "resumed",
      Self::Interrupted { .. } => "interrupted",
      Self::Continued { .. } => "continued",
      Self::Succeeded => "succeeded",
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
      Self::SpendRecorded { .. } => "spend_recorded",
    }
  }
}
