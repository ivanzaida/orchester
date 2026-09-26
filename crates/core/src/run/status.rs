use serde::{Deserialize, Serialize};

use crate::resource::StepKey;

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

/// A limit of a run: its token budget, its time limit on active agent time, or the project's
/// daily token budget that all its runs share.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLimit {
  Tokens,
  Time,
  /// The project's runs used today's token budget. The run continues with a plain resume once
  /// the day has room again (tomorrow, or a raised daily budget).
  DailyBudget,
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

/// A person's decision on a gated step's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanDecision {
  Approved,
  /// The step returns to work; its revised result needs a new decision.
  ChangesRequested,
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
