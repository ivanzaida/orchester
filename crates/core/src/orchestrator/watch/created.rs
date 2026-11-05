//! A task the leading conversation creates outside what it watches becomes a task subscription of it, so the end the
//! conversation waits for comes back to it. The new subscription takes the order of the subscription whose turn
//! created the task; a turn the person started has none, so it takes the default order.

use super::{Subscription, Subscriptions, WatchTarget};
use crate::{
  id::{ChatId, ModuleId, TaskId},
  time::Timestamp,
};

/// The task subscription `chat` gets, from `since` on, for the task `task` (in `module`, or in none) one of its turns
/// created: `None` when a subscription of `chat` in `subscriptions` already takes the task in. `turn_of` is the
/// subscription whose event turn created it, `None` for a turn of the person's (the default order).
pub fn created_task_watch(
  subscriptions: &Subscriptions,
  chat: ChatId,
  turn_of: Option<&Subscription>,
  task: TaskId,
  module: Option<ModuleId>,
  since: Timestamp,
) -> Option<Subscription> {
  if subscriptions
    .of_chat(chat)
    .any(|watch| watch.target().covers(task, module))
  {
    return None;
  }
  let order = turn_of.and_then(Subscription::order).unwrap_or_default();
  Some(Subscription::new(chat, WatchTarget::Task(task), order, since))
}
