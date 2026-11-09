//! The `/watching` answer (frame 06 `Ff7AD`): per subscription of a conversation, its target, kind, since when and
//! order, and the tasks it leads that have something going on — running, waiting for the person, queued, stopped at
//! a limit, ready to merge — with the rest counted, and a one-line total of what waits for the person. A module's
//! tasks are those in it now, the ones added after the subscription too. A task a more specific subscription leads
//! is listed under that one only, so no task is listed twice.

use std::sync::Arc;

use orchester_core::{
  id::{ChatId, TaskId},
  orchestrator::Subscription,
  run::{RunNumber, TaskPhase},
  task::Task,
  time::Timestamp,
};

use super::{LeadCounts, TargetView, WatchOrder, WatchReads, counted};
use crate::{
  links::{Link, TaskPage},
  messages::Message,
  tasks::needs_you::{WaitingItem, WaitingKind},
};

/// What a conversation leads and what waits there now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watching {
  /// How many of each kind of target it leads (*2 modules and 1 task*).
  pub counts: LeadCounts,
  /// The total, one part each (*1 thing waits for you*, *1 run is stopped at a limit*); *Nothing waits for you*
  /// when nothing does, or *This conversation watches nothing yet*.
  pub total: Vec<Message>,
  /// Its subscriptions, in the order they were kept.
  pub subscriptions: Vec<WatchedSubscription>,
}

/// One subscription of the conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchedSubscription {
  pub target: TargetView,
  pub since: Timestamp,
  pub order: WatchOrder,
  /// Its tasks that have something going on, in the project's order.
  pub tasks: Vec<WatchedTask>,
  /// Its other open tasks, counted (*4 more tasks with nothing under way*); `None` when there are none.
  pub rest: Option<Message>,
}

/// A task of a subscription that has something going on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchedTask {
  pub id: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  /// Its latest run, while it has one that can be read.
  pub run: Option<RunNumber>,
  pub going: Going,
  /// *Running*, *Needs input*.
  pub state: Message,
  /// What it waits for or where it stands (*its plan waits for you*); `None` for a running one.
  pub note: Option<Message>,
  /// The word of [`Self::link`] when it answers or decides (*Decide*, *Summary*, *Review*).
  pub action: Option<Message>,
  /// Where the person answers it, or else the task's page.
  pub link: String,
}

/// What is going on in a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Going {
  Running,
  Waits(WaitsFor),
  /// Waits in the queue for its run to start, at `position` from 1 among the project's queued tasks.
  Queued {
    position: usize,
  },
  StoppedAtLimit {
    daily_budget: bool,
  },
  ReadyToMerge,
}

/// What the person is waited for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitsFor {
  /// An agent's question, with suggested answers or in the conversation.
  Question,
  /// A message, with no question that can be read.
  Message,
  Plan,
  /// A tool request.
  Approval,
  /// A decision after a step's retries ran out.
  Decision,
  /// The run itself: recorded interrupted, left the merge queue, or committed nothing.
  Run,
}

impl WatchReads {
  /// What `chat` leads and what waits there now.
  pub fn watching(&self, chat: ChatId) -> Watching {
    let waiting = self.tasks.waiting();
    let queued = self.queued();
    let subscriptions: Vec<WatchedSubscription> = self
      .subscriptions
      .of_chat(chat)
      .map(|subscription| self.watched(subscription, &waiting, &queued))
      .collect();
    let total = total(&subscriptions);
    Watching {
      counts: LeadCounts::of(self.subscriptions.of_chat(chat)),
      total,
      subscriptions,
    }
  }

  fn watched(&self, subscription: &Subscription, waiting: &[WaitingItem], queued: &[TaskId]) -> WatchedSubscription {
    let mut tasks = Vec::new();
    let mut rest = 0;
    let led = self.tasks.tasks.iter().filter(|task| {
      !task.organization().status().is_closed()
        && self
          .leading(task)
          .is_some_and(|leading| leading.target() == subscription.target())
    });
    for task in led {
      match self.going(task, waiting, queued) {
        Some(going) => tasks.push(self.watched_task(task, going)),
        None => rest += 1,
      }
    }
    WatchedSubscription {
      target: self.target_view(subscription.target()),
      since: subscription.since(),
      order: self.order(subscription),
      tasks,
      rest: (rest > 0).then(|| counted(rest, ("orchestrator.watch.rest.one", "orchestrator.watch.rest.other"))),
    }
  }

  /// The project's open tasks that wait in the queue for their run to start, in the order they start.
  fn queued(&self) -> Vec<TaskId> {
    self
      .tasks
      .tasks
      .iter()
      .filter(|task| !task.organization().status().is_closed() && self.phase(task) == Some(TaskPhase::Queued))
      .map(Task::id)
      .collect()
  }

  /// What is going on in `task`: what of it waits for the person first, else its run at work or in the queue.
  fn going(&self, task: &Task, waiting: &[WaitingItem], queued: &[TaskId]) -> Option<Going> {
    let item = waiting
      .iter()
      .find(|item| item.task.as_ref().is_some_and(|waits| waits.id == task.id()));
    if let Some(item) = item {
      return Some(going_of(&item.kind));
    }
    match self.phase(task)? {
      TaskPhase::Running | TaskPhase::Waiting => Some(Going::Running),
      TaskPhase::Queued => queued
        .iter()
        .position(|id| *id == task.id())
        .map(|at| Going::Queued { position: at + 1 }),
      _ => None,
    }
  }

  fn watched_task(&self, task: &Task, going: Going) -> WatchedTask {
    let (state, note, action, page) = words(going);
    WatchedTask {
      id: task.id(),
      key: task.key().to_string().into(),
      title: task.details().title.as_str().into(),
      run: self
        .tasks
        .phases
        .get(&task.id())
        .and_then(|latest| latest.run())
        .map(|latest| latest.number),
      going,
      state: Message::new(state),
      note,
      action: action.map(Message::new),
      link: Link::Task(self.project, task.key().clone(), page).path(),
    }
  }
}

fn going_of(kind: &WaitingKind) -> Going {
  match kind {
    WaitingKind::Approval(_) => Going::Waits(WaitsFor::Approval),
    WaitingKind::Choice(_) | WaitingKind::Question(Some(_)) => Going::Waits(WaitsFor::Question),
    WaitingKind::Question(None) => Going::Waits(WaitsFor::Message),
    WaitingKind::Plan(_) => Going::Waits(WaitsFor::Plan),
    WaitingKind::Decision(_) => Going::Waits(WaitsFor::Decision),
    WaitingKind::StoppedAtLimit { daily_budget } => Going::StoppedAtLimit {
      daily_budget: *daily_budget,
    },
    WaitingKind::ReadyToMerge => Going::ReadyToMerge,
    WaitingKind::Interrupted { .. }
    | WaitingKind::ReadyToComplete
    | WaitingKind::CommittedNothing
    | WaitingKind::DeliveryLeft(_)
    | WaitingKind::DeliveryBlocked { .. }
    | WaitingKind::Memory { .. } => Going::Waits(WaitsFor::Run),
  }
}

/// The state's key, the note, the action's key and the page of the task's link of `going`.
fn words(going: Going) -> (&'static str, Option<Message>, Option<&'static str>, TaskPage) {
  let note = |key: &'static str| Some(Message::new(key));
  match going {
    Going::Running => (
      "orchestrator.watch.task.state.running",
      None,
      None,
      TaskPage::Conversation,
    ),
    Going::Waits(waits) => {
      let (key, action, page) = match waits {
        WaitsFor::Question => (
          "orchestrator.watch.task.note.question",
          "orchestrator.watch.task.action.answer",
          TaskPage::Conversation,
        ),
        WaitsFor::Message => (
          "orchestrator.watch.task.note.message",
          "orchestrator.watch.task.action.answer",
          TaskPage::Conversation,
        ),
        WaitsFor::Plan => (
          "orchestrator.watch.task.note.plan",
          "orchestrator.watch.task.action.decide",
          TaskPage::Conversation,
        ),
        WaitsFor::Approval => (
          "orchestrator.watch.task.note.approval",
          "orchestrator.watch.task.action.decide",
          TaskPage::Conversation,
        ),
        WaitsFor::Decision => (
          "orchestrator.watch.task.note.decision",
          "orchestrator.watch.task.action.decide",
          TaskPage::Summary,
        ),
        WaitsFor::Run => (
          "orchestrator.watch.task.note.run",
          "orchestrator.watch.task.action.summary",
          TaskPage::Summary,
        ),
      };
      ("orchestrator.watch.task.state.waits", note(key), Some(action), page)
    }
    Going::Queued { position } => (
      "orchestrator.watch.task.state.queued",
      Some(Message::new("orchestrator.watch.task.note.queued").with("position", position.to_string())),
      None,
      TaskPage::Summary,
    ),
    Going::StoppedAtLimit { daily_budget } => (
      "orchestrator.watch.task.state.stoppedAtLimit",
      note(if daily_budget {
        "orchestrator.watch.task.note.dailyBudget"
      } else {
        "orchestrator.watch.task.note.limit"
      }),
      Some("orchestrator.watch.task.action.summary"),
      TaskPage::Summary,
    ),
    Going::ReadyToMerge => (
      "orchestrator.watch.task.state.readyToMerge",
      note("orchestrator.watch.task.note.readyToMerge"),
      Some("orchestrator.watch.task.action.review"),
      TaskPage::Changes,
    ),
  }
}

/// The one-line total of `subscriptions`: what waits for the person and the runs stopped at a limit.
fn total(subscriptions: &[WatchedSubscription]) -> Vec<Message> {
  if subscriptions.is_empty() {
    return vec![Message::new("orchestrator.watch.total.nothing")];
  }
  let goings = || {
    subscriptions
      .iter()
      .flat_map(|watched| &watched.tasks)
      .map(|task| task.going)
  };
  let waits = goings().filter(|going| matches!(going, Going::Waits(_))).count();
  let limits = goings()
    .filter(|going| matches!(going, Going::StoppedAtLimit { .. }))
    .count();
  let mut total = Vec::new();
  if waits > 0 {
    total.push(counted(
      waits,
      (
        "orchestrator.watch.total.waits.one",
        "orchestrator.watch.total.waits.other",
      ),
    ));
  }
  if limits > 0 {
    total.push(counted(
      limits,
      (
        "orchestrator.watch.total.limits.one",
        "orchestrator.watch.total.limits.other",
      ),
    ));
  }
  if total.is_empty() {
    total.push(Message::new("orchestrator.watch.total.calm"));
  }
  total
}
