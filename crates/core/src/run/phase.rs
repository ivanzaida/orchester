//! Where a task stands, derived from the task, its latest run and that run's merge
//! (`docs/architecture/storage.md`, "State machines"). The phase is never stored: the task holds
//! the user's intent (draft or queued), the run holds what execution did and the merge record
//! whether its work reached the base branch.

use super::{
  state::Run,
  status::{PauseReason, RunStatus, StepStatus},
};
use crate::{
  integration::RunMerge,
  task::{Task, TaskStatus},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskPhase {
  Draft,
  /// Queued, and its run has not started yet.
  Queued,
  Running,
  /// Running, with a step waiting for a person (an approval, a plan decision or a message).
  Waiting,
  Paused,
  /// Reached its token or time limit and waits for the limit to be raised.
  StoppedAtLimit,
  /// Stopped without settling; its last checkpoint is kept.
  Interrupted,
  /// Every step passed; its work waits for review.
  Succeeded,
  /// Its run succeeded and its work was merged into the branch it started from.
  Merged,
  Failed,
  Cancelled,
}

impl TaskPhase {
  /// The phase of `task` whose most recent run is `latest`, merged as `merge` records.
  pub fn of(task: &Task, latest: Option<&Run>, merge: Option<&RunMerge>) -> Self {
    let Some(run) = latest else {
      return match task.status() {
        TaskStatus::Draft => Self::Draft,
        TaskStatus::Queued => Self::Queued,
      };
    };
    match run.status() {
      RunStatus::Pending => Self::Queued,
      RunStatus::Running if waiting(run) => Self::Waiting,
      RunStatus::Running => Self::Running,
      RunStatus::Paused(PauseReason::Requested) => Self::Paused,
      RunStatus::Paused(PauseReason::LimitReached(_)) => Self::StoppedAtLimit,
      RunStatus::Interrupted(_) => Self::Interrupted,
      RunStatus::Succeeded if merge.is_some_and(|merge| merge.run == run.id()) => Self::Merged,
      RunStatus::Succeeded => Self::Succeeded,
      RunStatus::Failed(_) => Self::Failed,
      RunStatus::Cancelled => Self::Cancelled,
    }
  }

  /// An agent may be working: the run has started and not settled.
  pub const fn is_active(self) -> bool {
    matches!(self, Self::Running | Self::Waiting)
  }

  /// The phase while no runner is connected to the project. A run recorded as running has lost
  /// its agent: it is interrupted and keeps its last checkpoint until a runner is back and
  /// records what happened (`docs/design/orchester-workspace.md`, "Runner connection"). A step
  /// waiting for a person has no agent at work, so it still waits.
  pub const fn without_runner(self) -> Self {
    match self {
      Self::Running => Self::Interrupted,
      other => other,
    }
  }
}

fn waiting(run: &Run) -> bool {
  run.current_attempt().is_some_and(|attempt| {
    attempt
      .executions()
      .iter()
      .any(|execution| matches!(execution.status(), StepStatus::Waiting(_)))
  })
}
