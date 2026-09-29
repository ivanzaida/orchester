//! The run's progress as the task page lists it (`qB7mj` in 02.12 `FjbKY`, `l3hyb` rows): one row
//! per step in stage order, with where its latest execution stands, its attempt, the tokens it
//! used and its active agent time. Parallel steps each retain their own row while waiting.

use std::{num::NonZeroU32, sync::Arc, time::Duration};

use orchester_core::{
  run::{PauseReason, RunStatus, StepFailure, StepStatus},
  time::Timestamp,
};

use super::detail::{RunView, StepView};

/// Where a step stands, as its row says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressState {
  Waiting,
  Working,
  /// Waiting for a person: an approval, a plan decision or a message.
  NeedsYou,
  /// A person paused the run while the step's agent worked.
  Paused,
  /// The run stopped at a limit (its budget or time limit, a step's budget, the project's daily
  /// budget) while the step's agent worked: "Stopped at limit", never "Paused" (02.L2 `A7nVbK`).
  StoppedAtLimit,
  /// The run lost its agent (no runner) or was interrupted mid-step.
  Interrupted,
  Complete,
  Failed(Option<StepFailure>),
  Skipped,
  Cancelled,
}

/// What an execution of the step used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionUsage {
  pub attempt: NonZeroU32,
  /// Counted tokens of all its invocations; `None` for a tool step, whose command uses none.
  pub tokens: Option<u64>,
  /// Active agent time of all its invocations.
  pub active: Duration,
}

/// One row of the progress list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressRow {
  /// Stable id of the row: its step key.
  pub id: Arc<str>,
  pub step: Arc<str>,
  pub agent: Option<Arc<str>>,
  pub state: ProgressState,
  pub usage: Option<ExecutionUsage>,
}

/// The progress rows of `run` at `now`. `lost`: the run is recorded as running but has no runner.
pub fn progress(run: &RunView, lost: bool, now: Timestamp) -> Vec<ProgressRow> {
  let mut ordered: Vec<&StepView> = run.steps.iter().collect();
  ordered.sort_by_key(|step| (step.stage, step.number));
  ordered
    .into_iter()
    .map(|step| ProgressRow {
      id: step.key.as_str().into(),
      step: step.name.clone(),
      agent: step.agent.clone(),
      state: state(run, step, lost),
      usage: usage(run, step, now),
    })
    .collect()
}

fn state(run: &RunView, step: &StepView, lost: bool) -> ProgressState {
  match (run.run.status(), step.status) {
    // Cancelled before its first attempt: no step will run, as if its executions were skipped.
    (RunStatus::Cancelled, None) => ProgressState::Cancelled,
    (_, None | Some(StepStatus::Pending)) => ProgressState::Waiting,
    (RunStatus::Paused(PauseReason::Requested), Some(StepStatus::Running)) => ProgressState::Paused,
    (RunStatus::Paused(PauseReason::LimitReached(_)), Some(StepStatus::Running)) => ProgressState::StoppedAtLimit,
    (RunStatus::Interrupted(_), Some(StepStatus::Running)) => ProgressState::Interrupted,
    (RunStatus::Running, Some(StepStatus::Running)) if lost => ProgressState::Interrupted,
    (_, Some(StepStatus::Running)) => ProgressState::Working,
    (_, Some(StepStatus::Waiting(_))) => ProgressState::NeedsYou,
    (_, Some(StepStatus::Passed)) => ProgressState::Complete,
    (_, Some(StepStatus::Failed)) => ProgressState::Failed(step.failure),
    (RunStatus::Cancelled, Some(StepStatus::Skipped)) => ProgressState::Cancelled,
    (_, Some(StepStatus::Skipped)) => ProgressState::Skipped,
  }
}

fn usage(run: &RunView, step: &StepView, now: Timestamp) -> Option<ExecutionUsage> {
  let execution = step.execution?;
  let invocations: Vec<_> = run.invocations_of(execution).collect();
  if invocations.is_empty() {
    return None;
  }
  let millis: i64 = invocations.iter().map(|invocation| invocation.active_millis(now)).sum();
  Some(ExecutionUsage {
    attempt: step.attempt?,
    tokens: (!step.tool_step).then(|| invocations.iter().map(|invocation| invocation.usage.counted()).sum()),
    active: Duration::from_millis(u64::try_from(millis).unwrap_or_default()),
  })
}
