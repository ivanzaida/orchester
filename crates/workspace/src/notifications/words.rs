//! What a notification says: what is needed in a few words, over the project, the task and the step it is about. It
//! shows on a locked screen, so it never carries a command, its arguments, a path, a file's contents, a message's
//! text or a secret: an event holds none of them.

use super::{Event, Kind, LimitKind, NotificationGroup, Origin, Target};
use crate::{messages::Message, tasks::list::DecisionWhy};

/// One OS notification: its title and body, the place a click opens, and whether it makes a sound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
  pub group: NotificationGroup,
  pub title: Message,
  pub body: Message,
  pub target: Target,
  pub sound: bool,
}

impl Notification {
  /// The notification of `group` for `events`, oldest first, each with how many more of its batch came with it. One
  /// event is told in full; several are counted, and their tasks named (*3 things need you · ORC-18, ORC-55 and 1
  /// more*). A click opens the oldest's place.
  pub(super) fn of(group: NotificationGroup, events: &[(&Event, usize)], sound: bool) -> Self {
    let (first, others) = events[0];
    let (title, body) = match events {
      [_] => (title(first, others), body(first)),
      _ => {
        let count: usize = events.iter().map(|(_, others)| others + 1).sum();
        (many(group, count), list(events))
      }
    };
    Self {
      group,
      title,
      body,
      target: first.target.clone(),
      sound,
    }
  }
}

/// What `event` needs, or what happened, with `others` more runs of its batch.
fn title(event: &Event, others: usize) -> Message {
  let key = match &event.kind {
    Kind::Approval { command: true } => "notifications.title.command",
    Kind::Approval { command: false } => "notifications.title.tool",
    Kind::Question { agent } => {
      return Message::new("notifications.title.question").with("agent", agent.to_string());
    }
    Kind::Message => "notifications.title.message",
    Kind::Plan { version } => {
      return Message::new("notifications.title.plan").with("version", version.to_string());
    }
    Kind::Decision(why) => decision(*why),
    Kind::Limit(Some(limit)) if others > 0 => {
      return Message::new(limit_many(*limit)).with("count", (others + 1).to_string());
    }
    Kind::Limit(Some(limit)) => limit_one(*limit),
    Kind::Limit(None) => "notifications.title.limit.any",
    Kind::Interrupted => "notifications.title.interrupted",
    Kind::RunnerOffline => "notifications.title.runnerOffline",
    Kind::RunnerNotStarted => "notifications.title.runnerNotStarted",
    Kind::MergeConflicts => "notifications.title.mergeConflicts",
    Kind::LocalIntegration => "notifications.title.localIntegration",
    Kind::ReadyToMerge => "notifications.title.readyToMerge",
    Kind::Merged => "notifications.title.merged",
    Kind::Failed => "notifications.title.failed",
    Kind::PlanLimitNear { percent } => {
      return Message::new("notifications.title.planLimitNear").with("percent", percent.to_string());
    }
    Kind::PlanLimitReset => "notifications.title.planLimitReset",
    Kind::Proposal => "notifications.title.proposal",
  };
  Message::new(key)
}

const fn decision(why: DecisionWhy) -> &'static str {
  match why {
    DecisionWhy::RetriesExhausted => "notifications.title.retries",
    DecisionWhy::BrokeAgain => "notifications.title.brokeAgain",
    DecisionWhy::Unverified => "notifications.title.unverified",
    DecisionWhy::Unchanged => "notifications.title.unchanged",
  }
}

const fn limit_one(limit: LimitKind) -> &'static str {
  match limit {
    LimitKind::Tokens => "notifications.title.limit.tokens",
    LimitKind::Time => "notifications.title.limit.time",
    LimitKind::DailyBudget => "notifications.title.limit.daily",
    LimitKind::GroupBudget => "notifications.title.limit.group",
    LimitKind::StepBudget => "notifications.title.limit.step",
    LimitKind::PlanLimit => "notifications.title.limit.plan",
  }
}

/// A batch is only ever a shared budget's; another limit is told as one run's.
const fn limit_many(limit: LimitKind) -> &'static str {
  match limit {
    LimitKind::GroupBudget => "notifications.title.limit.groupMany",
    _ => "notifications.title.limit.dailyMany",
  }
}

/// The project, the task and the step `event` is about; the account, for a plan limit.
fn body(event: &Event) -> Message {
  let project = match &event.origin {
    Origin::Account { name } => return Message::new("notifications.body.account").with("account", name.to_string()),
    Origin::Project { name, .. } => name.to_string(),
  };
  let Some(task) = &event.task else {
    return Message::new("notifications.body.project").with("project", project);
  };
  let message = |key| {
    Message::new(key)
      .with("project", project.clone())
      .with("task", task.key.to_string())
      .with("title", task.title.to_string())
  };
  match &event.step {
    Some(step) => message("notifications.body.taskStep").with("step", step.to_string()),
    None => message("notifications.body.task"),
  }
}

/// How many things of `group` happened.
fn many(group: NotificationGroup, count: usize) -> Message {
  let key = match group {
    NotificationGroup::NeedsYou => "notifications.many.needsYou",
    NotificationGroup::Stopped => "notifications.many.stopped",
    NotificationGroup::Finished => "notifications.many.finished",
  };
  Message::new(key).with("count", count.to_string())
}

/// The tasks several events are about, the first two by key and the rest counted, after their project when they
/// share one; an event about no task is named by its project, or its account.
fn list(events: &[(&Event, usize)]) -> Message {
  let mut names: Vec<String> = Vec::new();
  for (event, _) in events {
    let name = match (&event.task, &event.origin) {
      (Some(task), _) => task.key.to_string(),
      (None, Origin::Project { name, .. } | Origin::Account { name }) => name.to_string(),
    };
    if !names.contains(&name) {
      names.push(name);
    }
  }
  let project = shared_project(events);
  let (one, two, more) = if project.is_some() {
    (
      "notifications.list.project.one",
      "notifications.list.project.two",
      "notifications.list.project.more",
    )
  } else {
    (
      "notifications.list.across.one",
      "notifications.list.across.two",
      "notifications.list.across.more",
    )
  };
  let (first, rest) = names
    .split_first()
    .expect("each event names one thing, and there are several");
  let message = match rest {
    [] => Message::new(one).with("first", first.clone()),
    [second] => Message::new(two)
      .with("first", first.clone())
      .with("second", second.clone()),
    [second, more_names @ ..] => Message::new(more)
      .with("first", first.clone())
      .with("second", second.clone())
      .with("count", more_names.len().to_string()),
  };
  match project {
    Some(project) => message.with("project", project),
    None => message,
  }
}

/// The name of the project every one of `events` is about, about a task of it; `None` when they are about several,
/// or about an account.
fn shared_project(events: &[(&Event, usize)]) -> Option<String> {
  let mut names = events.iter().map(|(event, _)| match &event.origin {
    Origin::Project { name, .. } if event.task.is_some() => Some(name),
    Origin::Project { .. } | Origin::Account { .. } => None,
  });
  let first = names.next().flatten()?;
  names.all(|name| name == Some(first)).then(|| first.to_string())
}
