//! A run waits on another task of its project (ORC-787; the Orchestrator module's design, screen 04): the
//! Orchestrator creates a task, and a run that waits for a message or a decision waits on it until that task's work
//! is merged; the merge is an event for the waiting run's task, on which the Orchestrator's turn continues the run.
//!
//! No git work of its own: the waited task's merge lands in the waiting run's base branch, so the run's branch lacks
//! it like any other move of its base, and the base-moved path (`crate::base_move`, `super::prompt::base_moved`)
//! brings it into the run's worktree when the run continues: the runner merges the moved base before the agent reads
//! the notice.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{branch::BranchName, status::WaitReason};
use crate::{
  id::{ProjectId, RunId, TaskId},
  task::organization::WorkflowStatus,
};

/// Why a run cannot wait on a task.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum WaitsOnError {
  #[error("a run cannot wait on its own task")]
  OwnTask,
  #[error("a run waits only on a task of its own project")]
  OtherProject,
  /// A tool call's access approval is answered at once; only a message or a decision may take as long as a task.
  #[error("only a run that waits for a message or a decision can wait on a task")]
  NotWaitingForPerson,
  #[error("the run already waits on task {0}")]
  AlreadyWaits(TaskId),
}

/// The run that is to wait: its project, its task, the branch its work merges into, and what its step waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingRun {
  pub project: ProjectId,
  pub run: RunId,
  pub task: TaskId,
  pub base: BranchName,
  pub waits_for: WaitReason,
}

/// Run `run` of task `task` waits on task `waited` of the same project, until `waited`'s work is merged into `base`,
/// the run's base branch. One task per run ([`RunWaitsOn::check_only`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunWaitsOn {
  run: RunId,
  task: TaskId,
  base: BranchName,
  waited: TaskId,
}

/// What happened that may end a wait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitsOnChange {
  /// Task `task`'s work was merged into `branch`.
  Merged { task: TaskId, branch: BranchName },
  /// Task `task`'s workflow status became `status`. A merge that finished it ends the wait as [`Self::Merged`] first,
  /// so a close the wait still sees was done without a merge into the run's base.
  StatusChanged { task: TaskId, status: WorkflowStatus },
  /// Run `run` ended: it succeeded, failed or was canceled.
  RunEnded { run: RunId },
}

/// How a wait ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitsOnEnd {
  /// An event for the waiting run's task.
  Event(WaitedTaskEvent),
  /// The waiting run ended: the wait goes without a word.
  Dropped,
}

/// What became of a waited task, for the task whose run waits on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitedTaskEvent {
  /// The waiting run's task, which the event is for.
  pub task: TaskId,
  pub run: RunId,
  pub waited: TaskId,
  pub kind: WaitedTaskEventKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WaitedTaskEventKind {
  /// Its work was merged into the waiting run's base branch: the run continues, and the base-moved path brings the
  /// merge into its worktree.
  Merged { branch: BranchName },
  /// It was canceled.
  Canceled,
  /// It was closed as done without its work being merged into the waiting run's base branch.
  ClosedUnmerged,
}

impl WaitedTaskEventKind {
  /// Whether the event is told to the person: the waited work will not land, so the run does not go on by itself.
  pub const fn tells_person(&self) -> bool {
    matches!(self, Self::Canceled | Self::ClosedUnmerged)
  }
}

impl RunWaitsOn {
  /// `run` waits on `waited`, a task of `waited_project`: refused for the run's own task, a task of another project,
  /// or a run that does not wait for a message or a decision.
  pub fn new(run: WaitingRun, waited: TaskId, waited_project: ProjectId) -> Result<Self, WaitsOnError> {
    if waited == run.task {
      return Err(WaitsOnError::OwnTask);
    }
    if waited_project != run.project {
      return Err(WaitsOnError::OtherProject);
    }
    if !matches!(run.waits_for, WaitReason::Message | WaitReason::Plan) {
      return Err(WaitsOnError::NotWaitingForPerson);
    }
    Ok(Self {
      run: run.run,
      task: run.task,
      base: run.base,
      waited,
    })
  }

  /// Whether it may be kept beside `waits`, those kept already: its run waits on no task yet.
  pub fn check_only(&self, waits: &[Self]) -> Result<(), WaitsOnError> {
    match waits.iter().find(|wait| wait.run == self.run) {
      Some(kept) => Err(WaitsOnError::AlreadyWaits(kept.waited)),
      None => Ok(()),
    }
  }

  pub const fn run(&self) -> RunId {
    self.run
  }

  pub const fn task(&self) -> TaskId {
    self.task
  }

  pub const fn base(&self) -> &BranchName {
    &self.base
  }

  pub const fn waited(&self) -> TaskId {
    self.waited
  }

  /// How `change` ends the wait; `None` when the wait goes on: it is about another task or run, a merge into another
  /// branch, or a status that does not close the task.
  pub fn end(&self, change: &WaitsOnChange) -> Option<WaitsOnEnd> {
    let kind = match change {
      WaitsOnChange::RunEnded { run } => return (*run == self.run).then_some(WaitsOnEnd::Dropped),
      WaitsOnChange::Merged { task, branch } if *task == self.waited && *branch == self.base => {
        WaitedTaskEventKind::Merged { branch: branch.clone() }
      }
      WaitsOnChange::StatusChanged { task, status } if *task == self.waited => match status {
        WorkflowStatus::Canceled => WaitedTaskEventKind::Canceled,
        WorkflowStatus::Done => WaitedTaskEventKind::ClosedUnmerged,
        WorkflowStatus::Backlog | WorkflowStatus::Todo | WorkflowStatus::InProgress | WorkflowStatus::InReview => {
          return None;
        }
      },
      WaitsOnChange::Merged { .. } | WaitsOnChange::StatusChanged { .. } => return None,
    };
    Some(WaitsOnEnd::Event(WaitedTaskEvent {
      task: self.task,
      run: self.run,
      waited: self.waited,
      kind,
    }))
  }
}
