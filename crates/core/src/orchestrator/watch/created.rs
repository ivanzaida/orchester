//! A task the leading conversation creates outside what it watches becomes a task subscription of it, so the end the
//! conversation waits for comes back to it. The new subscription takes the order of the subscription whose turn
//! created the task; a turn the person started has none, so it takes the default order. A conversation that watches
//! nothing gets none, and neither does one whose task another conversation leads more specifically than the project
//! already (the task's module, or the tasks without a module): that one keeps it, and the action says who leads it.

use serde::{Deserialize, Serialize};

use super::{Subscription, Subscriptions, WatchTarget};
use crate::{
  id::{ChatId, ModuleId, TaskId},
  time::Timestamp,
};

/// What a task one of a conversation's turns created means for what the conversation watches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Created {
  /// The conversation watches nothing, or one of its subscriptions takes the task in already: nothing is added.
  Unchanged,
  /// The task becomes this task subscription of the conversation.
  Watch(Subscription),
  /// Another conversation leads the task's module, or the tasks without a module, with this subscription: nothing
  /// is added, and the task's events go there.
  LedElsewhere(Subscription),
}

/// What the task `task` (in `module`, or in none) one of `chat`'s turns created, at `since`, means ([`Created`]).
/// `turn_of` is the subscription whose event started the turn, `None` for a turn of the person's (the default order).
pub fn created_task(
  subscriptions: &Subscriptions,
  chat: ChatId,
  turn_of: Option<&Subscription>,
  (task, module): (TaskId, Option<ModuleId>),
  since: Timestamp,
) -> Created {
  let mut own = subscriptions.of_chat(chat).peekable();
  if own.peek().is_none() || own.any(|watch| watch.target().covers(task, module)) {
    return Created::Unchanged;
  }
  // A new task has no subscription of its own: only the project's is less specific than the one it would get.
  let inner = module.map_or(WatchTarget::WithoutModule, WatchTarget::Module);
  if let Some(led) = subscriptions.of(inner) {
    return Created::LedElsewhere(led.clone());
  }
  let order = turn_of.and_then(Subscription::order).unwrap_or_default();
  Created::Watch(Subscription::new(chat, WatchTarget::Task(task), order, since))
}

/// The task subscription `chat` gets, from `since` on, for the task `task` (in `module`, or in none) one of its turns
/// created ([`created_task`]): `None` when it gets none.
pub fn created_task_watch(
  subscriptions: &Subscriptions,
  chat: ChatId,
  turn_of: Option<&Subscription>,
  task: TaskId,
  module: Option<ModuleId>,
  since: Timestamp,
) -> Option<Subscription> {
  match created_task(subscriptions, chat, turn_of, (task, module), since) {
    Created::Watch(subscription) => Some(subscription),
    Created::Unchanged | Created::LedElsewhere(_) => None,
  }
}

/// The subscription of `chat` an event of the task `task` (in `module`, or in none) came through: the most specific
/// of its own that takes the task in; `None` when none does.
pub fn event_subscription(
  subscriptions: &Subscriptions,
  chat: ChatId,
  task: TaskId,
  module: Option<ModuleId>,
) -> Option<&Subscription> {
  WatchTarget::of_task(task, module)
    .into_iter()
    .find_map(|target| subscriptions.of(target).filter(|led| led.chat() == chat))
}

/// What became of the subscription a created task's action names, as its row says it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreatedWatch {
  /// The task became a task subscription of the conversation: *watched here*.
  WatchedHere,
  /// Another conversation, `chat` titled `title` then, leads the task's module or the tasks without a module: the
  /// task's events go there.
  LedElsewhere { chat: ChatId, title: String },
}
