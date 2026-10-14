//! The new Overview (`Overview` `vgqpM`) as data, beside what [`super::overview`] counts for every
//! overview: its task tabs and their rows, its figures (`Metrics` `anUOY`) and the notices under its
//! heading (`Notices` `pk5Jk`). A task is in *Needs you* while anything of it waits for the person, as
//! [`ProjectTasks::waiting`] lists it and [`super::attention`] words it; a run interrupted is both in
//! *Running*, where it was at work, and in *Needs you*, since it waits for the person to continue it.

use std::{num::NonZeroU32, sync::Arc};

use orchester_core::{id::TaskId, run::TaskPhase, task::Task, time::Timestamp, tokens::compact_tokens};

use super::{
  attention::{self, NeedItem, NeedKind},
  data::ProjectTasks,
  list::{Latest, TaskRow, phase_of},
  overview::Pulse,
  usage::ProjectUsage,
};
use crate::messages::Message;

/// A tab of the Overview's task list (`Tabs` in `xwXYK`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardTab {
  Running,
  NeedsYou,
  Queued,
  Failed,
}

impl BoardTab {
  pub const ALL: [Self; 4] = [Self::Running, Self::NeedsYou, Self::Queued, Self::Failed];

  /// Whether `task` is in the tab: in *Needs you* while one of `waiting` is about it, in the others by
  /// its phase; one whose latest run cannot be read is in none of those.
  fn holds(self, task: &Task, tasks: &ProjectTasks, waiting: &[NeedItem]) -> bool {
    let phase = phase_of(task, &tasks.phases);
    match self {
      Self::Running => matches!(phase, Some(TaskPhase::Running | TaskPhase::Interrupted)),
      Self::NeedsYou => waiting
        .iter()
        .any(|item| item.task().is_some_and(|waits| waits.id == task.id())),
      Self::Queued => matches!(phase, Some(TaskPhase::Queued)),
      Self::Failed => matches!(phase, Some(TaskPhase::Failed)),
    }
  }

  /// What waits for the person, when the tab needs it.
  fn waiting(self, tasks: &ProjectTasks) -> Vec<NeedItem> {
    if self == Self::NeedsYou {
      attention::items(tasks)
    } else {
      Vec::new()
    }
  }

  /// Tasks in the tab; in *Needs you* what every *Needs you* counts ([`attention::count`]): each waiting
  /// object, so a task waiting on several steps and a request whose task cannot be read count as the pill
  /// and the *Need your input* figure count them.
  pub fn count(self, tasks: &ProjectTasks) -> usize {
    if self == Self::NeedsYou {
      return attention::count(tasks);
    }
    tasks.tasks.iter().filter(|task| self.holds(task, tasks, &[])).count()
  }

  /// The tab's lines that are no task's, after its tasks and outside their limit: in *Needs you* the
  /// project's memory records waiting for approval (`Row · project memory` `S9Zgf`), *Not a task:
  /// Kontur memory* with `project` the project's name.
  pub fn project_rows(self, tasks: &ProjectTasks, project: &str) -> Vec<ProjectRow> {
    self
      .waiting(tasks)
      .into_iter()
      .filter(|item| item.kind == NeedKind::Memory)
      .map(|item| ProjectRow {
        detail: Message::new("overview.board.memoryRow").with("project", project.to_owned()),
        item,
      })
      .collect()
  }

  /// The newest `limit` tasks of the tab, newest first, with their tags and what blocks them, blocking
  /// tasks' keys joined by `separator`. In *Needs you* a row says what waits, as its oldest item in
  /// [`attention::items`] does (*Plan v2 from Planner waits for your decision*).
  pub fn rows(self, tasks: &ProjectTasks, limit: usize, separator: &str) -> Vec<TaskRow> {
    let waiting = self.waiting(tasks);
    tasks
      .tasks
      .iter()
      .rev()
      .filter(|task| self.holds(task, tasks, &waiting))
      .take(limit)
      .map(|task| {
        let mut row = tasks.decorate_row(
          TaskRow::new(task, &tasks.pipelines, tasks.phases.get(&task.id())),
          separator,
        );
        if let Some(item) = waiting
          .iter()
          .find(|item| item.task().is_some_and(|waits| waits.id == task.id()))
        {
          row.detail = item.title.clone();
        }
        row
      })
      .collect()
  }
}

/// A line of a tab that is no task's: what waits, worded as its card ([`NeedItem::title`]), and what it
/// is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRow {
  pub item: NeedItem,
  pub detail: Message,
}

/// The Overview's figures, counted from the tasks' latest runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Figures {
  /// Runs with an agent at work.
  pub running: usize,
  /// Runs interrupted: they lost their agent and wait to continue.
  pub interrupted: usize,
  /// Things waiting for the person ([`attention::items`]).
  pub needs_you: usize,
  /// Runs that passed every step since the day began.
  pub completed_today: usize,
  /// Whether any task has a run at all.
  pub any_run: bool,
}

impl Figures {
  /// `day_start` is local midnight of today.
  pub fn of(tasks: &ProjectTasks, day_start: Timestamp) -> Self {
    let pulse = Pulse::of(&tasks.phases, day_start);
    let interrupted = tasks
      .phases
      .values()
      .filter(|latest| latest.phase() == Some(TaskPhase::Interrupted))
      .count();
    Self {
      running: pulse.running,
      interrupted,
      needs_you: attention::count(tasks),
      completed_today: pulse.completed_today,
      any_run: !tasks.phases.is_empty(),
    }
  }

  /// *Running now* (`EsfZQ`): with `online` whether a runner serves the project and `agents` the agents
  /// at work of its slots as it last reported them.
  pub fn running_now(&self, online: bool, agents: Option<(usize, NonZeroU32)>) -> RunningNow {
    if self.interrupted > 0 {
      return RunningNow {
        count: Some(self.running + self.interrupted),
        detail: Pulse::running_context(false, None, self.interrupted),
        interrupted: true,
      };
    }
    let detail = match (online, self.any_run) {
      (true, false) => Message::new("overview.board.noRuns"),
      (true, true) => Pulse::running_context(true, agents, 0),
      (false, _) => Message::new("overview.metrics.agentsUnknown"),
    };
    RunningNow {
      count: online.then_some(self.running),
      detail,
      interrupted: false,
    }
  }
}

/// What *Running now* shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningNow {
  /// The figure; `None` while no runner reports what runs.
  pub count: Option<usize>,
  pub detail: Message,
  /// Runs were interrupted: the figure counts them and calls for the person.
  pub interrupted: bool,
}

/// *Tokens today* (`AyOf1`): today's tokens, compact, and the project's daily budget under them; `None`
/// figure while the usage is not read. `project` names the project when it has no daily budget.
pub fn tokens_today(usage: Option<&ProjectUsage>, project: &str) -> (Option<String>, Message) {
  let Some(usage) = usage else {
    return (None, Message::new("overview.board.usageUnread"));
  };
  let detail = match usage.limits.daily_budget_limit() {
    Some(budget) => Message::new("overview.board.ofBudget").with("budget", compact_tokens(budget.get())),
    None => Message::new("overview.board.noBudget").with("project", project.to_owned()),
  };
  (Some(compact_tokens(usage.today.get())), detail)
}

/// What a notice about a task says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
  /// Other tasks block it.
  Blocked,
  /// Its latest run cannot be read.
  Unreadable,
}

/// A notice under the Overview's heading about one task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskNotice {
  pub kind: NoticeKind,
  pub task: TaskId,
  pub key: Arc<str>,
  pub title: Message,
  pub body: Message,
}

/// The tasks other tasks block, each with what it waits for (*Blocked · KON-7*), then the tasks whose
/// latest run cannot be read (*No run to show for KON-21*); blocking tasks' keys joined by `separator`.
pub fn notices(tasks: &ProjectTasks, separator: &str) -> Vec<TaskNotice> {
  let blocked = tasks
    .tasks
    .iter()
    .filter(|task| tasks.blocking.get(&task.id()).is_some_and(|links| !links.is_empty()))
    .map(|task| {
      let row = tasks.decorate_row(
        TaskRow::new(task, &tasks.pipelines, tasks.phases.get(&task.id())),
        separator,
      );
      notice(NoticeKind::Blocked, task.id(), &row.key, row.detail)
    });
  let unreadable = tasks
    .tasks
    .iter()
    .filter(|task| matches!(tasks.phases.get(&task.id()), Some(Latest::Unreadable(_))))
    .map(|task| {
      let key = task.key().to_string();
      let body = Message::new("overview.board.unreadable.body").with("task", key.clone());
      notice(NoticeKind::Unreadable, task.id(), &key, body)
    });
  blocked.chain(unreadable).collect()
}

fn notice(kind: NoticeKind, task: TaskId, key: &str, body: Message) -> TaskNotice {
  let title = match kind {
    NoticeKind::Blocked => "overview.board.blocked.title",
    NoticeKind::Unreadable => "overview.board.unreadable.title",
  };
  TaskNotice {
    kind,
    task,
    key: key.into(),
    title: Message::new(title).with("task", key.to_owned()),
    body,
  }
}
