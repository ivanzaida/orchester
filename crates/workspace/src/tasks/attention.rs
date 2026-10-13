//! What waits for the person in a project as the new Overview words it, one item each (its *Needs you*
//! column `NbKDR`, its tab and its *Need your input* figure). What waits, and so what every *Needs you*
//! counts, is [`ProjectTasks::waiting`]: this only names each object for the Overview's cards, in the same
//! order, oldest wait first.

use std::time::Duration;

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
  /// The kind's word, over the item (`Kind` in `rkhZ2`).
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
}

/// One thing waiting for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedItem {
  pub kind: NeedKind,
  /// The task it is about; `None` for the project's memory, and for a tool request whose run or task
  /// cannot be read any more, which still waits and still counts.
  pub task: Option<WaitingTask>,
  /// Since when it waits: when the request was made, or when the run last changed.
  pub since: Timestamp,
  /// What waits, in a line (`Plan v2 from Planner waits for your decision`).
  pub title: Message,
}

impl NeedItem {
  /// The task it is about: *KON-16 · Rate-limit messages from GitHub*, the project's memory, or that it
  /// is not known.
  pub fn context(&self) -> Message {
    match (&self.task, self.kind) {
      (Some(task), _) => Message::new("overview.needs.context")
        .with("task", task.key.to_string())
        .with("title", task.title.to_string()),
      (None, NeedKind::Memory) => Message::new("overview.needs.contextMemory"),
      (None, _) => Message::new("overview.needs.contextUnread"),
    }
  }

  /// How long it waits at `now`; an interrupted run waits since it stopped.
  pub fn waited(&self, now: Timestamp) -> Waited {
    if self.kind == NeedKind::Interrupted {
      return Waited::Since(display::clock(self.since));
    }
    // A clock that went backwards reads as no wait.
    let millis = u64::try_from(now.unix_millis() - self.since.unix_millis()).unwrap_or_default();
    Waited::For(display::duration(Duration::from_millis(millis)))
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
  tasks.waiting().into_iter().map(|item| need(tasks, item)).collect()
}

/// How many things wait for the person in `tasks`: what every *Needs you* counts.
pub fn count(tasks: &ProjectTasks) -> usize {
  tasks.needs_you()
}

fn need(tasks: &ProjectTasks, item: WaitingItem) -> NeedItem {
  let number = item.run.map(|(_, number)| number);
  let run = |key: &'static str| run_message(key, number);
  let (kind, title) = match &item.kind {
    WaitingKind::Approval(shown) => {
      let request = &shown.approval;
      let title = if request.command {
        Message::new("overview.needs.title.command").with("command", shown.first_line().0.to_owned())
      } else {
        Message::new("overview.needs.title.tool")
          .with("tool", request.action.to_string())
          .with("server", request.server.to_string())
      };
      (NeedKind::Approval, title.with("agent", request.agent.to_string()))
    }
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
  };
  NeedItem {
    kind,
    task: item.task,
    since: item.since,
    title,
  }
}

fn question(agent: &str, text: &str) -> Message {
  Message::new("overview.needs.title.question")
    .with("agent", agent.to_owned())
    .with("question", text.to_owned())
}

/// `key`'s words about the item's run, numbered `number` (*Run #142 was interrupted*).
fn run_message(key: &'static str, number: Option<RunNumber>) -> Message {
  Message::new(key).with("run", number.map(|number| number.get().to_string()).unwrap_or_default())
}
