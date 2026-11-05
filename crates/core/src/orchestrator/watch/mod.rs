//! The rules of a conversation's subscriptions that need no store (the orchestrator-watch design frame "What the
//! design decides", rows Ending and Created tasks): when a subscription ends by itself ([`ending`]), what a person's
//! stop leaves open ([`stop`]), and which task the leading conversation creates becomes a task subscription of it
//! ([`created`]). They read plain inputs (a target, task statuses and phases), so the stored subscriptions only have
//! to say what they watch and with which order.

pub mod created;
pub mod ending;
pub mod stop;

pub use self::{
  created::created_task_watch,
  ending::{EndReason, Ending, NeverEnds, TargetTasks},
  stop::{LeftOpen, OpenWork},
};
use crate::id::{ModuleId, TaskId};

/// What a subscription watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WatchTarget {
  Task(TaskId),
  /// The module's tasks, those added to it later too.
  Module(ModuleId),
  /// The project's tasks that are in no module.
  WithoutModule,
  /// Every task of the project.
  Project,
}

impl WatchTarget {
  /// Whether the target takes in the task `task`, which is in `module` or in none.
  pub fn covers(self, task: TaskId, module: Option<ModuleId>) -> bool {
    match self {
      Self::Task(watched) => watched == task,
      Self::Module(watched) => module == Some(watched),
      Self::WithoutModule => module.is_none(),
      Self::Project => true,
    }
  }
}

/// One subscription of a conversation: its target and its order, the text after the chips. No order takes the default
/// one of Project settings › Orchestrator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watch {
  pub target: WatchTarget,
  pub order: Option<String>,
}
