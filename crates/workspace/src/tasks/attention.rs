//! What waits for the person in a project, one item each (the new Overview's *Needs you* column `NbKDR`,
//! its tab and its *Need your input* figure): a tool request waiting for a decision, a question, a plan to
//! review, a decision after a step's retries, a run stopped at its limit, a run interrupted, and a run
//! whose every step passed and whose work waits to be reviewed and merged. Interrupted runs count: they
//! wait for the person to continue them. The oldest wait comes first.

use std::{sync::Arc, time::Duration};

use orchester_core::{
  id::TaskId,
  run::{RunNumber, TaskPhase},
  task::Task,
  time::Timestamp,
  tokens::compact_tokens,
};

use super::{
  approvals::ShownApproval,
  data::ProjectTasks,
  list::{LatestRun, phase_of},
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
    })
  }

  /// The kind a task in `phase` waits as, without a tool request of its own; `None` when it waits for
  /// nothing.
  const fn of_phase(phase: TaskPhase) -> Option<Self> {
    match phase {
      TaskPhase::Waiting => Some(Self::Input),
      TaskPhase::Stuck => Some(Self::Decision),
      TaskPhase::StoppedAtLimit => Some(Self::StoppedAtLimit),
      TaskPhase::Interrupted => Some(Self::Interrupted),
      TaskPhase::Succeeded => Some(Self::ReadyToMerge),
      TaskPhase::Draft
      | TaskPhase::Queued
      | TaskPhase::Running
      | TaskPhase::Paused
      | TaskPhase::Merged
      | TaskPhase::Failed
      | TaskPhase::Cancelled => None,
    }
  }
}

/// Whether a task in `phase` waits for the person; one whose latest run cannot be read does not.
pub const fn waits(phase: Option<TaskPhase>) -> bool {
  match phase {
    Some(phase) => NeedKind::of_phase(phase).is_some(),
    None => false,
  }
}

/// One thing waiting for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedItem {
  pub kind: NeedKind,
  /// The task it is about; `None` for a tool request whose run or task cannot be read any more, which
  /// still waits and still counts.
  pub task: Option<NeedTask>,
  /// Since when it waits: when the request was made, or when the run last changed.
  pub since: Timestamp,
  /// What waits, in a line (`Plan v2 from Planner waits for your decision`).
  pub title: Message,
}

/// The task an item is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeedTask {
  pub id: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
}

impl NeedItem {
  /// The task it is about: *KON-16 · Rate-limit messages from GitHub*, or that it is not known.
  pub fn context(&self) -> Message {
    match &self.task {
      Some(task) => Message::new("overview.needs.context")
        .with("task", task.key.to_string())
        .with("title", task.title.to_string()),
      None => Message::new("overview.needs.contextUnread"),
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

/// Everything waiting for the person in `tasks`, the oldest wait first. A task whose tool requests are
/// listed is not listed again for the wait they cause.
pub fn items(tasks: &ProjectTasks) -> Vec<NeedItem> {
  let mut items: Vec<NeedItem> = tasks
    .pending_tool_approvals
    .iter()
    .map(|shown| approval(tasks, shown))
    .collect();
  let asking: Vec<TaskId> = items.iter().filter_map(|item| Some(item.task.as_ref()?.id)).collect();
  items.extend(
    tasks
      .tasks
      .iter()
      .filter(|task| !asking.contains(&task.id()))
      .filter_map(|task| waiting_task(task, tasks)),
  );
  items.sort_by_key(|item| item.since);
  items
}

/// How many things wait for the person in `tasks`: [`items`] counted.
pub fn count(tasks: &ProjectTasks) -> usize {
  items(tasks).len()
}

/// A tool request as an item, without its task when that cannot be read any more: the request still waits.
fn approval(tasks: &ProjectTasks, shown: &ShownApproval) -> NeedItem {
  let task = tasks.tasks.iter().find(|task| Some(task.id()) == shown.task_id);
  let request = &shown.approval;
  let title = if request.command {
    Message::new("overview.needs.title.command").with("command", shown.first_line().0.to_owned())
  } else {
    Message::new("overview.needs.title.tool")
      .with("tool", request.action.to_string())
      .with("server", request.server.to_string())
  };
  item(
    task,
    NeedKind::Approval,
    request.requested_at,
    title.with("agent", request.agent.to_string()),
  )
}

/// The task as an item, when its latest run waits for the person.
fn waiting_task(task: &Task, tasks: &ProjectTasks) -> Option<NeedItem> {
  let kind = NeedKind::of_phase(phase_of(task, &tasks.phases)?)?;
  let latest = tasks.phases.get(&task.id())?.run()?;
  let (kind, title) = match (kind, &latest.plan, &latest.question) {
    (NeedKind::Input, Some(plan), _) => (
      NeedKind::PlanReview,
      Message::new("overview.needs.title.plan")
        .with("version", plan.version.to_string())
        .with("agent", plan.agent.to_string()),
    ),
    (NeedKind::Input, None, Some(question)) => (
      NeedKind::Question,
      Message::new("overview.needs.title.question")
        .with("agent", question.agent.to_string())
        .with("question", question.text.to_string()),
    ),
    (kind, ..) => (kind, run_title(kind, task, latest)),
  };
  Some(item(Some(task), kind, latest.updated_at, title))
}

/// What a run waits for, when no plan, question or request says it.
fn run_title(kind: NeedKind, task: &Task, latest: &LatestRun) -> Message {
  let run = |key: &'static str, number: RunNumber| Message::new(key).with("run", number.get().to_string());
  match kind {
    NeedKind::Decision => latest.decision.as_ref().map_or_else(
      || run("overview.needs.title.input", latest.number),
      |decision| decision.title(),
    ),
    NeedKind::StoppedAtLimit if latest.daily_budget => run("overview.needs.title.dailyBudget", latest.number),
    NeedKind::StoppedAtLimit => match latest.token_limit {
      Some(limit) => run("overview.needs.title.tokenLimit", latest.number).with("limit", compact_tokens(limit)),
      None => run("overview.needs.title.stoppedAtLimit", latest.number),
    },
    NeedKind::Interrupted => run("overview.needs.title.interrupted", latest.number),
    NeedKind::ReadyToMerge => Message::new("overview.needs.title.readyToMerge").with("task", task.key().to_string()),
    NeedKind::Approval | NeedKind::Question | NeedKind::PlanReview | NeedKind::Input => {
      run("overview.needs.title.input", latest.number)
    }
  }
}

fn item(task: Option<&Task>, kind: NeedKind, since: Timestamp, title: Message) -> NeedItem {
  NeedItem {
    kind,
    task: task.map(|task| NeedTask {
      id: task.id(),
      key: task.key().to_string().into(),
      title: task.details().title.as_str().into(),
    }),
    since,
    title,
  }
}
