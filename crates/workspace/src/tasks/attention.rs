//! What waits for the person in a project as people read it, one item each: the one wording of the new
//! Overview's *Needs you* column (`NbKDR`), its tab and its *Need your input* figure, and of the title bar's
//! Needs you panel (`bAeM6`). What waits, and so what every *Needs you* counts, is [`ProjectTasks::waiting`]:
//! this names each object, says how long it waits and what its action is, and where it is decided, in the
//! same order, oldest wait first.

use std::sync::Arc;

use orchester_core::{run::RunNumber, time::Timestamp, tokens::compact_tokens};

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
      Self::Memory => "overview.needs.kind.memory",
    })
  }

  /// The words of the item's action: *Review*, *Answer*, *Review plan*, *Raise the limit*.
  pub fn action(self) -> Message {
    Message::new(match self {
      Self::Approval => "overview.needs.action.review",
      Self::Question => "overview.needs.action.answer",
      Self::PlanReview => "overview.needs.action.reviewPlan",
      Self::Decision => "overview.needs.action.decide",
      Self::Input | Self::Interrupted => "overview.needs.action.openTask",
      Self::StoppedAtLimit => "overview.needs.action.raiseLimit",
      Self::ReadyToMerge => "overview.needs.action.reviewMerge",
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

  /// The task it is about: *KON-16 · Rate-limit messages from GitHub*, the project's memory, or that it
  /// is not known.
  pub fn context(&self) -> Message {
    match (self.task(), self.kind) {
      (Some(task), _) => Message::new("overview.needs.context")
        .with("task", task.key.to_string())
        .with("title", task.title.to_string()),
      (None, NeedKind::Memory) => Message::new("overview.needs.contextMemory"),
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
  /// records and what has no task to open nowhere yet.
  pub fn decided(&self) -> Decided {
    if self.kind == NeedKind::Memory {
      return Decided::Unavailable(Message::new("overview.needs.unavailable.memory"));
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
      NeedKind::Approval | NeedKind::Interrupted | NeedKind::ReadyToMerge | NeedKind::Memory => Decided::Summary(None),
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
    WaitingKind::Memory { count: 1 } => (NeedKind::Memory, Message::new("overview.needs.title.memoryOne")),
    WaitingKind::Memory { count } => (
      NeedKind::Memory,
      Message::new("overview.needs.title.memoryOther").with("count", count.to_string()),
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
