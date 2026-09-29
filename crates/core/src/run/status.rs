use serde::{Deserialize, Serialize};

use crate::{id::StepExecutionId, resource::StepKey};

/// Why a running run stopped without a checkpoint-safe pause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptCause {
  /// The runner process dropped while the application kept running. Runs interrupted this
  /// way continue on their own once the runner reconnects.
  RunnerLost,
  /// The user chose Quit now while agents were working.
  QuitNow,
  /// The application ended abnormally (crash, power loss, process killed).
  Crashed,
}

impl InterruptCause {
  /// Whether a reconnecting runner continues the run without asking. Runs interrupted by the
  /// end of a session wait for an explicit continue.
  pub const fn continues_on_reconnect(self) -> bool {
    matches!(self, Self::RunnerLost)
  }
}

/// Why a run failed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunFailure {
  /// The step failed and its pipeline step has no failure route.
  StepFailed(StepKey),
  /// The step failed after its failure route was taken `max_retries` times.
  RetriesExhausted(StepKey),
}

/// A limit of a run: its token budget, its time limit on active agent time, the project's daily
/// token budget that all its runs share, or the budget of one of its steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLimit {
  Tokens,
  Time,
  /// The project's runs used today's token budget. The run continues with a plain resume once
  /// the day has room again (tomorrow, or a raised daily budget).
  DailyBudget,
  /// `execution` used its step's budget. The run continues once that step's budget is raised for
  /// the run ([`super::Run::raise_step_budget`]); the published pipeline stays as it is.
  StepBudget {
    execution: StepExecutionId,
  },
}

impl RunLimit {
  /// Whether this is a limit of the whole run, which a raised run budget or time limit lifts, as
  /// opposed to a step's budget.
  pub const fn is_run_limit(self) -> bool {
    !matches!(self, Self::StepBudget { .. })
  }
}

/// Why a run is paused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
  /// A person paused it (or chose to wait for agents to save before closing).
  #[default]
  Requested,
  /// It reached this limit, its agents were stopped and its work committed: "Stopped at limit". It
  /// continues only after the limit is raised; after the daily budget, once the day has room.
  LimitReached(RunLimit),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RunStatus {
  Pending,
  Running,
  Paused(PauseReason),
  /// Stopped without settling; the last checkpoint is kept and nothing after it counts as done.
  Interrupted(InterruptCause),
  Succeeded,
  Failed(RunFailure),
  Cancelled,
}

impl RunStatus {
  pub const fn is_terminal(&self) -> bool {
    matches!(self, Self::Succeeded | Self::Failed(_) | Self::Cancelled)
  }

  /// A run that failed or was cancelled is final; its task can only run again ([`super::run_again`]).
  pub const fn can_run_again(&self) -> bool {
    matches!(self, Self::Failed(_) | Self::Cancelled)
  }
}

/// What a waiting step waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitReason {
  /// A tool call awaits an access approval.
  Approval,
  /// The step's result awaits a decision at its approval gate, e.g. plan approval.
  Plan,
  /// The agent asked a question and awaits a message.
  Message,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepStatus {
  Pending,
  Running,
  Waiting(WaitReason),
  Passed,
  Failed,
  /// Will not run to completion in its attempt: the attempt ended by retry, failure or cancel.
  Skipped,
}

impl StepStatus {
  pub const fn is_open(self) -> bool {
    matches!(self, Self::Pending | Self::Running | Self::Waiting(_))
  }
}

/// Why a step execution failed. Its step's failure route decides what happens next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepFailure {
  /// The agent CLI did not finish the step: a turn failed, it exited with a failure or it
  /// reported no result.
  AgentFailed,
  /// The agent finished and reported that it could not do the step.
  ReportedFailed,
  /// The agent, or a command tool's program, could not be started.
  NotStarted,
  /// Legacy: versions before step budgets stopped the run at its limit failed an execution that
  /// used its step's budget. Such failures are still read; none is recorded any more.
  StepBudget,
  /// The step's changes conflict with what other steps changed on the run's branch.
  MergeConflict,
  /// The step's changes could not be committed or merged.
  CheckpointFailed,
  /// A command tool's program exited with a code the tool does not expect, or did not finish
  /// within the tool's timeout.
  CommandFailed,
}

/// Why an attempt was started.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptReason {
  Initial,
  /// `failed_step` failed and its failure route returned to `return_to`.
  Retry {
    failed_step: StepKey,
    return_to: StepKey,
  },
  /// The run had succeeded and a person asked the agent of `step` about it (Ask agent): its one
  /// execution continues the session of the step's passed one ([`super::Run::follow_up`]).
  FollowUp {
    step: StepKey,
  },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttemptStatus {
  Active,
  /// A failure was routed back to an upstream step in a newer attempt.
  RoutedBack,
  Succeeded,
  Failed,
  Cancelled,
}

/// Who continued an interrupted run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinueTrigger {
  User,
  RunnerReconnected,
}
