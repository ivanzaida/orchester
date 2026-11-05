//! What the project's Orchestrator conversations watch (`/watch-task`, `/watch-module`): each subscription names a
//! conversation, a target and the person's order for it. An event of a task goes to exactly one conversation, the
//! most specific subscription that covers the task ([`Subscriptions::leading`]): the task's own, then its module's (or
//! the one of tasks without a module, for a task with none), then the project's. With none, callers keep the rule of
//! [`super::wait::dealt_with`], else the project's latest conversation. One conversation leads each target at most,
//! which [`Subscriptions`] keeps, so two subscriptions never tie.
//!
//! The rules that need no store beside them (the orchestrator-watch design frame "What the design decides", rows
//! Ending and Created tasks): when a subscription ends by itself ([`ending`]), what a person's stop leaves open
//! ([`stop`]), and which task the leading conversation creates becomes a task subscription of it ([`created`]).

mod change;
pub mod created;
mod ended;
pub mod ending;
pub mod stop;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  change::{News, Stopped, Taken, Told},
  created::created_task_watch,
  ended::WatchEnd,
  ending::{EndReason, Ending, NeverEnds, TargetTasks},
  stop::{LeftOpen, OpenWork},
};
use crate::{
  id::{ChatId, ModuleId, TaskId},
  time::Timestamp,
};

/// What a conversation can watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum WatchTarget {
  Task(TaskId),
  /// A module, with the tasks added to it later.
  Module(ModuleId),
  /// The project's tasks that have no module.
  WithoutModule,
  /// Every task of the project.
  Project,
}

impl WatchTarget {
  /// The targets that cover a task in `module` (`None`: it has none), most specific first.
  pub fn of_task(task: TaskId, module: Option<ModuleId>) -> [Self; 3] {
    [
      Self::Task(task),
      module.map_or(Self::WithoutModule, Self::Module),
      Self::Project,
    ]
  }

  /// Whether the target takes in the task `task`, which is in `module` or in none.
  pub fn covers(self, task: TaskId, module: Option<ModuleId>) -> bool {
    match self {
      Self::Task(watched) => watched == task,
      Self::Module(watched) => module == Some(watched),
      Self::WithoutModule => module.is_none(),
      Self::Project => true,
    }
  }

  /// The less specific targets that cover this one, nearest first; `task_module` is the module of a task target
  /// (`None`: it has none) and says nothing for the other targets.
  fn outer(self, task_module: Option<ModuleId>) -> Vec<Self> {
    match self {
      Self::Task(task) => Self::of_task(task, task_module)[1..].to_vec(),
      Self::Module(_) | Self::WithoutModule => vec![Self::Project],
      Self::Project => Vec::new(),
    }
  }
}

/// A conversation's subscription to a target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
  chat: ChatId,
  target: WatchTarget,
  order: String,
  since: Timestamp,
}

impl Subscription {
  /// `chat` watches `target` from `since` on, following `order`, the person's text; a blank one takes the project's
  /// default order.
  pub fn new(chat: ChatId, target: WatchTarget, order: impl Into<String>, since: Timestamp) -> Self {
    let order = order.into().trim().to_owned();
    Self {
      chat,
      target,
      order,
      since,
    }
  }

  pub const fn chat(&self) -> ChatId {
    self.chat
  }

  pub const fn target(&self) -> WatchTarget {
    self.target
  }

  /// The person's order; `None` when the project's default order applies.
  pub fn order(&self) -> Option<&str> {
    (!self.order.is_empty()).then_some(self.order.as_str())
  }

  pub const fn since(&self) -> Timestamp {
    self.since
  }
}

/// Two kept subscriptions name the same target: one conversation leads a target at most.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("two conversations lead the watch target {target:?}")]
pub struct TwoLeaders {
  pub target: WatchTarget,
}

/// The project's subscriptions, at most one per target.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Subscriptions {
  kept: Vec<Subscription>,
}

/// What a /watch of a target by a conversation means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
  /// Nobody leads the target: the subscription is added. `takes_from` is the other conversation that leads the
  /// target's tasks now through a less specific subscription (a task's module, or the project), and loses them.
  New { takes_from: Option<ChatId> },
  /// The conversation leads the target already.
  AlreadyHere,
  /// Another conversation leads the target, since `since`: the person opens it or takes the target over.
  LedElsewhere { chat: ChatId, since: Timestamp },
}

impl Subscriptions {
  /// The subscriptions as kept; refused when two of them name the same target.
  pub fn new(kept: impl IntoIterator<Item = Subscription>) -> Result<Self, TwoLeaders> {
    let mut set = Self::default();
    for subscription in kept {
      if set.of(subscription.target).is_some() {
        return Err(TwoLeaders {
          target: subscription.target,
        });
      }
      set.kept.push(subscription);
    }
    Ok(set)
  }

  pub fn as_slice(&self) -> &[Subscription] {
    &self.kept
  }

  /// The subscription that leads `target`, if any.
  pub fn of(&self, target: WatchTarget) -> Option<&Subscription> {
    self.kept.iter().find(|subscription| subscription.target == target)
  }

  /// The subscriptions of `chat`, in the order they were kept.
  pub fn of_chat(&self, chat: ChatId) -> impl Iterator<Item = &Subscription> {
    self.kept.iter().filter(move |subscription| subscription.chat == chat)
  }

  /// The conversation an event of `task`, in `module` (`None`: it has none), goes to: the most specific
  /// subscription that covers it. `None`: no subscription does.
  pub fn leading(&self, task: TaskId, module: Option<ModuleId>) -> Option<ChatId> {
    self
      .first_of(WatchTarget::of_task(task, module))
      .map(Subscription::chat)
  }

  /// What a /watch of `target` by `chat` means; `task_module` is the module of a task target (`None`: it has none)
  /// and says nothing for the other targets.
  pub fn claim(&self, chat: ChatId, target: WatchTarget, task_module: Option<ModuleId>) -> Claim {
    match self.of(target) {
      Some(led) if led.chat == chat => Claim::AlreadyHere,
      Some(led) => Claim::LedElsewhere {
        chat: led.chat,
        since: led.since,
      },
      None => Claim::New {
        takes_from: self.outer_leader(target, task_module).filter(|&other| other != chat),
      },
    }
  }

  /// The conversation that leads `target`'s tasks through a less specific subscription.
  fn outer_leader(&self, target: WatchTarget, task_module: Option<ModuleId>) -> Option<ChatId> {
    self.first_of(target.outer(task_module)).map(Subscription::chat)
  }

  fn first_of(&self, targets: impl IntoIterator<Item = WatchTarget>) -> Option<&Subscription> {
    targets.into_iter().find_map(|target| self.of(target))
  }
}
