//! *Landed*: the merge queue's state in a line, then the runs merged in Orchester today and in the week before, newest
//! first, each at its merge's time. Landed counts merges only: a run that passed every step and waits to be merged is
//! a wait ([`crate::tasks::attention`]), or the merge queue's while the queue delivers it.

use std::sync::Arc;

use orchester_core::{
  id::TaskId,
  merge_queue::{QueueEntry, QueueState},
  run::TaskPhase,
  time::Timestamp,
};

use crate::{messages::Message, tasks::data::ProjectTasks};

/// How far back *Earlier* reaches: the week before today.
const EARLIER_DAYS: i64 = 7;
const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandedRun {
  pub task: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub at: Timestamp,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Landed {
  /// Merged since `day_start`, newest first.
  pub today: Vec<LandedRun>,
  /// Merged in the week before, newest first.
  pub earlier: Vec<LandedRun>,
}

/// The merges of `tasks`' latest runs, `day_start` being local midnight of today.
pub fn landed(tasks: &ProjectTasks, day_start: Timestamp) -> Landed {
  let since = Timestamp::from_unix_millis(day_start.unix_millis() - EARLIER_DAYS * DAY_MILLIS);
  let mut merged: Vec<LandedRun> = tasks
    .tasks
    .iter()
    .filter_map(|task| {
      let latest = tasks.phases.get(&task.id())?.run()?;
      if latest.phase != TaskPhase::Merged {
        return None;
      }
      let at = *tasks.run_facts.merged_at.get(&latest.id)?;
      Some(LandedRun {
        task: task.id(),
        key: task.key().to_string().into(),
        title: task.details().title.as_str().into(),
        at,
      })
    })
    .filter(|run| run.at >= since)
    .collect();
  merged.sort_by_key(|run| std::cmp::Reverse(run.at));
  let (today, earlier) = merged.into_iter().partition(|run| run.at >= day_start);
  Landed { today, earlier }
}

/// The merge queue in a line: its state (*empty*, *KON-26 rechecking*, *blocked: your checkout keeps delivery out*)
/// and how many runs wait behind it (*1 waits*), when any do.
pub fn queue_line(tasks: &ProjectTasks) -> (Message, Option<Message>) {
  let entries = &tasks.merge_queue;
  let waiting = |head: Option<&QueueEntry>| {
    let count = entries
      .iter()
      .filter(|entry| entry.state == QueueState::Queued && head.is_none_or(|head| head.run != entry.run))
      .count();
    let key = if count == 1 {
      "overview.landed.waiting.one"
    } else {
      "overview.landed.waiting.other"
    };
    (count > 0).then(|| Message::new(key).with("count", count.to_string()))
  };
  if entries
    .iter()
    .any(|entry| matches!(entry.state, QueueState::Blocked(_)))
  {
    return (Message::new("overview.landed.state.blocked"), waiting(None));
  }
  let head = entries
    .iter()
    .find(|entry| !matches!(entry.state, QueueState::Queued | QueueState::Removed))
    .or_else(|| entries.iter().find(|entry| entry.state == QueueState::Queued));
  let Some(head) = head else {
    return (Message::new("overview.landed.empty"), None);
  };
  let key = match &head.state {
    QueueState::Queued => "overview.landed.state.queued",
    QueueState::Syncing => "overview.landed.state.syncing",
    QueueState::Rechecking { .. } => "overview.landed.state.rechecking",
    QueueState::Integrating { .. } => "overview.landed.state.integrating",
    QueueState::WaitingForApproval => "overview.landed.state.approval",
    QueueState::Left(_) | QueueState::Blocked(_) | QueueState::Merged(_) | QueueState::Removed => {
      "overview.landed.state.left"
    }
  };
  let task = tasks
    .tasks
    .iter()
    .find(|task| task.id() == head.task)
    .map(|task| task.key().to_string())
    .unwrap_or_default();
  (Message::new(key).with("task", task), waiting(Some(head)))
}
