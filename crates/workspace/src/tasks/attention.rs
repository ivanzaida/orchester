//! What waits for the person in a project as people read it, one item each: the one wording of the new
//! Overview's *Needs you* column (`NbKDR`), its tab and its *Need your input* figure, and of the title bar's
//! Needs you panel (`bAeM6`). What waits, and so what every *Needs you* counts, is [`ProjectTasks::waiting`]:
//! this names each object, says how long it waits and what its action is, and where it is decided, in the
//! same order, oldest wait first.

use std::sync::Arc;

use orchester_core::{
  merge_queue::{Blocked, Left},
  run::RunNumber,
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{
  data::ProjectTasks,
  needs_you::{WaitingItem, WaitingKind, WaitingTask},
};
use crate::{display, messages::Message};

/// What an item waits for the person to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeedKind {
  /// A tool request waiting for a decision.
  Approval,
  /// An agent's question waiting for an answer.
  Question,
  /// A plan waiting for a decision.
  PlanReview,
  /// A run waiting for a decision after its step's retries ran out or it broke again.
  Decision,
  /// A run waiting for the person in another way: its request is a card of the task's conversation.
  Input,
  /// A run stopped at its token or time limit, or at the project's daily budget.
  StoppedAtLimit,
  /// A run that stopped without settling; it continues from its last checkpoint.
  Interrupted,
  /// A run whose every step passed: its work waits to be reviewed and merged.
  ReadyToMerge,
  /// A run whose every step passed and committed nothing, whose result is its report: it waits to be completed.
  ReadyToComplete,
  /// A run whose every step passed and committed nothing although its pipeline expects changes.
  CommittedNothing,
  /// A run out of the merge queue with conflicts its Integrate step could not resolve.
  Conflicts,
  /// A run out of the merge queue for another reason: a failed check on the merged result, a base that kept
  /// moving, delivery that stopped.
  DeliveryStopped,
  /// The merge queue, blocked by the person's checkout.
  DeliveryBlocked,
  /// The project's memory records waiting for the person's approval.
  Memory,
}

impl NeedKind {
  /// The kind's word, over the item (`Kind` in `rkhZ2`, `x4f2I` in `bAeM6`).
  pub fn label(self) -> Message {
    Message::new(match self {
      Self::Approval => "overview.needs.kind.approval",
      Self::Question => "overview.needs.kind.question",
      Self::PlanReview => "overview.needs.kind.planReview",
      Self::Decision => "overview.needs.kind.decision",
      Self::Input => "overview.needs.kind.input",
      Self::StoppedAtLimit => "overview.needs.kind.stoppedAtLimit",
      Self::Interrupted => "overview.needs.kind.interrupted",
      Self::ReadyToMerge => "overview.needs.kind.readyToMerge",
      Self::ReadyToComplete => "overview.needs.kind.readyToComplete",
      Self::CommittedNothing => "overview.needs.kind.committedNothing",
      Self::Conflicts => "overview.needs.kind.conflicts",
      Self::DeliveryStopped => "overview.needs.kind.deliveryStopped",
      Self::DeliveryBlocked => "overview.needs.kind.deliveryBlocked",
      Self::Memory => "overview.needs.kind.memory",
    })
  }

  /// The words of the item's action: *Review*, *Answer*, *Review plan*, *Raise the limit*.
  pub fn action(self) -> Message {
    Message::new(match self {
      Self::Approval => "overview.needs.action.review",
      Self::Question => "overview.needs.action.answer",
      Self::PlanReview => "overview.needs.action.reviewPlan",
      Self::Decision | Self::CommittedNothing => "overview.needs.action.decide",
      Self::Input | Self::Interrupted | Self::DeliveryStopped => "overview.needs.action.openTask",
      Self::StoppedAtLimit => "overview.needs.action.raiseLimit",
      Self::ReadyToMerge => "overview.needs.action.reviewMerge",
      Self::ReadyToComplete => "overview.needs.action.reviewComplete",
      Self::Conflicts => "overview.needs.action.openConflicts",
      Self::DeliveryBlocked => "overview.needs.action.openMergeQueue",
      Self::Memory => "overview.needs.action.reviewRecords",
    })
  }
}

/// A decision card on a task's Summary that an item is decided at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryCard {
  /// The plan under review.
  Plan,
  /// Raising the limit the run stopped at.
  RaiseLimit,
  /// The decision after a step's retries.
  Retries,
}

/// Where an item is decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decided {
  /// At its card, keyed so, in the run's conversation: a tool request, a question with suggested answers. The
  /// Needs you panel answers it there in place.
  Card(Arc<str>),
  /// In the task's conversation, where the composer answers its agent.
  Conversation,
  /// On the task's Summary, at its decision card when it has one.
  Summary(Option<SummaryCard>),
  /// On the project's Memory page, in its *Waiting for approval* queue.
  Memory,
  /// On the project's Merge queue screen.
  MergeQueue,
  /// Nowhere it can be opened, and why.
  Unavailable(Message),
}

/// One thing waiting for the person, worded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedItem {
  pub kind: NeedKind,
  /// What waits, in a line (`Plan v2 from Planner waits for your decision`).
  pub title: Message,
  /// The object itself.
  pub waiting: WaitingItem,
}

impl NeedItem {
  /// `item`, one of what waits in `tasks`, worded.
  pub fn of(tasks: &ProjectTasks, item: WaitingItem) -> Self {
    let (kind, title) = words(tasks, &item);
    Self {
      kind,
      title,
      waiting: item,
    }
  }

  /// The task it is about; `None` for the project's memory, and for a tool request whose run or task cannot
  /// be read any more, which still waits and still counts.
  pub const fn task(&self) -> Option<&WaitingTask> {
    self.waiting.task.as_ref()
  }

  /// Since when it waits: when the request was made, or when the run last changed.
  pub const fn since(&self) -> Timestamp {
    self.waiting.since
  }

  /// The task it is about: *KON-16 · Rate-limit messages from GitHub*, the project's memory, the merge queue, or
  /// that it is not known.
  pub fn context(&self) -> Message {
    match (self.task(), &self.waiting.kind) {
      (Some(task), _) => Message::new("overview.needs.context")
        .with("task", task.key.to_string())
        .with("title", task.title.to_string()),
      (None, WaitingKind::Memory { .. }) => Message::new("overview.needs.contextMemory"),
      (None, WaitingKind::DeliveryBlocked { waiting, .. }) => {
        Message::new("overview.needs.contextMergeQueue").with("count", waiting.to_string())
      }
      (None, _) => Message::new("overview.needs.contextUnread"),
    }
  }

  /// How long it waits at `now`, in its largest whole unit (*41 min*, *3 h*, *2 d*); an interrupted run
  /// waits since it stopped.
  pub fn waited(&self, now: Timestamp) -> Waited {
    if self.kind == NeedKind::Interrupted {
      return Waited::Since(display::clock(self.since()));
    }
    Waited::For(display::waited(now, self.since()))
  }

  /// [`Self::waited`] in words, *waiting 41 min* or *since 16:47*, with `read` giving the words of a wait.
  pub fn wait_line(&self, now: Timestamp, read: impl FnOnce(&Message) -> String) -> Message {
    match self.waited(now) {
      Waited::For(time) => Message::new("overview.needs.waiting").with("time", read(&time)),
      Waited::Since(clock) => Message::new("overview.needs.since").with("time", clock),
    }
  }

  /// Where it is decided: a request at its card, an agent's question in the conversation, a plan, a limit
  /// and a decision after retries at their card of the Summary, the rest on the Summary; the project's memory
  /// records on its Memory page, a blocked merge queue on its screen; what has no task to open nowhere.
  pub fn decided(&self) -> Decided {
    match self.kind {
      NeedKind::Memory => return Decided::Memory,
      NeedKind::DeliveryBlocked => return Decided::MergeQueue,
      _ => {}
    }
    if self.task().is_none() {
      return Decided::Unavailable(Message::new("overview.needs.unavailable.task"));
    }
    if let Some(card) = self.waiting.kind.card_key() {
      return Decided::Card(card);
    }
    match self.kind {
      NeedKind::Question | NeedKind::Input => Decided::Conversation,
      NeedKind::PlanReview => Decided::Summary(Some(SummaryCard::Plan)),
      NeedKind::StoppedAtLimit => Decided::Summary(Some(SummaryCard::RaiseLimit)),
      NeedKind::Decision => Decided::Summary(Some(SummaryCard::Retries)),
      NeedKind::DeliveryBlocked => Decided::MergeQueue,
      NeedKind::Approval
      | NeedKind::Interrupted
      | NeedKind::ReadyToMerge
      | NeedKind::ReadyToComplete
      | NeedKind::CommittedNothing
      | NeedKind::Conflicts
      | NeedKind::DeliveryStopped
      | NeedKind::Memory => Decided::Summary(None),
    }
  }
}

/// How long an item waits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waited {
  /// For this long (*41 min*).
  For(Message),
  /// Since this local time (*16:47*).
  Since(String),
}

/// Everything waiting for the person in `tasks` ([`ProjectTasks::waiting`]), the oldest wait first.
pub fn items(tasks: &ProjectTasks) -> Vec<NeedItem> {
  tasks
    .waiting()
    .iter()
    .map(|item| NeedItem::of(tasks, item.clone()))
    .collect()
}

/// How many things wait for the person in `tasks`: what every *Needs you* counts.
pub fn count(tasks: &ProjectTasks) -> usize {
  tasks.needs_you()
}

/// What waits for the person in `tasks` and is no task's — the project's memory records, a tool request whose
/// task cannot be read — in one line, for the task list's *Needs you* tab, which lists tasks only: the item's
/// own words when it is one (*1 memory record waits for approval*), how many when several; `None` when
/// everything that waits is a task's.
pub fn outside_tasks(tasks: &ProjectTasks) -> Option<Message> {
  let waiting = tasks.waiting();
  let outside: Vec<&WaitingItem> = waiting.iter().filter(|item| item.task.is_none()).collect();
  match outside.as_slice() {
    [] => None,
    [item] => Some(words(tasks, item).1),
    several => Some(Message::new("tasks.needsYou.outsideTasks").with("count", several.len().to_string())),
  }
}

fn words(tasks: &ProjectTasks, item: &WaitingItem) -> (NeedKind, Message) {
  let number = item.run.map(|(_, number)| number);
  let run = |key: &'static str| run_message(key, number);
  match &item.kind {
    WaitingKind::Approval(shown) => {
      let request = &shown.approval;
      let title = if request.command {
        let (line, more) = shown.first_line();
        let key = if more {
          "overview.needs.title.commandMore"
        } else {
          "overview.needs.title.command"
        };
        Message::new(key).with("command", line.to_owned())
      } else {
        Message::new("overview.needs.title.tool")
          .with("tool", request.action.to_string())
          .with("server", request.server.to_string())
      };
      (NeedKind::Approval, title.with("agent", request.agent.to_string()))
    }
    WaitingKind::Choice(choice) if choice.count > 1 => (
      NeedKind::Question,
      Message::new("overview.needs.title.questions")
        .with("agent", choice.agent.to_string())
        .with("count", choice.count.to_string()),
    ),
    WaitingKind::Choice(choice) => (NeedKind::Question, question(&choice.agent, &choice.first)),
    WaitingKind::Question(Some(asked)) => (NeedKind::Question, question(&asked.agent, &asked.text)),
    WaitingKind::Question(None) => (NeedKind::Input, run("overview.needs.title.input")),
    WaitingKind::Plan(plan) => (
      NeedKind::PlanReview,
      Message::new("overview.needs.title.plan")
        .with("version", plan.version.to_string())
        .with("agent", plan.agent.to_string()),
    ),
    WaitingKind::Decision(decision) => (NeedKind::Decision, decision.title()),
    WaitingKind::Interrupted { .. } => (NeedKind::Interrupted, run("overview.needs.title.interrupted")),
    WaitingKind::StoppedAtLimit { daily_budget: true } => {
      (NeedKind::StoppedAtLimit, run("overview.needs.title.dailyBudget"))
    }
    WaitingKind::StoppedAtLimit { daily_budget: false } => {
      let limit = item
        .task
        .as_ref()
        .and_then(|task| tasks.phases.get(&task.id)?.run()?.token_limit);
      let title = match limit {
        Some(limit) => run("overview.needs.title.tokenLimit").with("limit", compact_tokens(limit)),
        None => run("overview.needs.title.stoppedAtLimit"),
      };
      (NeedKind::StoppedAtLimit, title)
    }
    WaitingKind::ReadyToMerge => (
      NeedKind::ReadyToMerge,
      Message::new("overview.needs.title.readyToMerge").with(
        "task",
        item.task.as_ref().map(|task| task.key.to_string()).unwrap_or_default(),
      ),
    ),
    WaitingKind::ReadyToComplete => (NeedKind::ReadyToComplete, run("overview.needs.title.readyToComplete")),
    WaitingKind::CommittedNothing => (NeedKind::CommittedNothing, run("overview.needs.title.committedNothing")),
    WaitingKind::DeliveryLeft(left) => delivery_left(left, number),
    WaitingKind::DeliveryBlocked { blocked, .. } => (
      NeedKind::DeliveryBlocked,
      Message::new(match blocked {
        Blocked::Uncommitted { .. } => "overview.needs.title.blockedUncommitted",
        Blocked::OtherBranch { .. } => "overview.needs.title.blockedOtherBranch",
        Blocked::InProgress => "overview.needs.title.blockedInProgress",
        Blocked::HeldLocally => "overview.needs.title.blockedHeld",
      }),
    ),
    WaitingKind::Memory { count: 1 } => (NeedKind::Memory, Message::new("overview.needs.title.memoryOne")),
    WaitingKind::Memory { count } => (
      NeedKind::Memory,
      Message::new("overview.needs.title.memoryOther").with("count", count.to_string()),
    ),
  }
}

/// Why run `number` left the merge queue without being merged.
fn delivery_left(left: &Left, number: Option<RunNumber>) -> (NeedKind, Message) {
  match left {
    Left::Conflicts { files } => (
      NeedKind::Conflicts,
      Message::new("overview.needs.title.conflicts").with("files", files.join(", ")),
    ),
    Left::CheckFailed { tool, .. } => (
      NeedKind::DeliveryStopped,
      Message::new("overview.needs.title.checkFailed").with("tool", tool.clone()),
    ),
    Left::KeepsMoving { base, heads } => (
      NeedKind::DeliveryStopped,
      Message::new("overview.needs.title.keepsMoving")
        .with("base", base.clone())
        .with("count", heads.len().to_string()),
    ),
    Left::Stopped { detail } => (
      NeedKind::DeliveryStopped,
      run_message("overview.needs.title.deliveryStopped", number)
        .with("detail", detail.lines().next().unwrap_or_default().to_owned()),
    ),
    Left::LaterRun => (
      NeedKind::DeliveryStopped,
      run_message("overview.needs.title.laterRun", number),
    ),
  }
}

/// *Explorer asks: index the vendored crates?*, the question's first line.
fn question(agent: &str, text: &str) -> Message {
  let first = text.lines().find(|line| !line.trim().is_empty()).unwrap_or_default();
  Message::new("overview.needs.title.question")
    .with("agent", agent.to_owned())
    .with("question", first.trim().to_owned())
}

/// `key`'s words about the item's run, numbered `number` (*Run #142 was interrupted*).
fn run_message(key: &'static str, number: Option<RunNumber>) -> Message {
  Message::new(key).with("run", number.map(|number| number.get().to_string()).unwrap_or_default())
}
