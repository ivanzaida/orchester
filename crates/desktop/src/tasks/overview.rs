//! The project overview (02.08 `Twtqc`) as data: the project pulse (`C95Z93`), the task tabs
//! (`i2uFe`) with their rows and footer (`WV44u`), what needs the user (`EKUWS`) and the recent
//! run activity (`YJ8PV`). Everything is counted from the project's tasks and their latest runs.

use std::sync::Arc;

use orchester_core::{
  id::TaskId,
  run::{RunNumber, TaskPhase},
  task::Task,
  time::Timestamp,
};

use super::{
  data::PipelineChoice,
  list::{Phases, TaskRow},
};
use crate::messages::Message;

/// Tabs of the overview's task list (`i2uFe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewTab {
  All,
  Running,
  NeedsYou,
  Queued,
}

impl lurq::app::component::DevtoolsInspectable for OverviewTab {}

impl OverviewTab {
  pub const ALL: [Self; 4] = [Self::All, Self::Running, Self::NeedsYou, Self::Queued];

  pub const fn label_key(self) -> &'static str {
    match self {
      Self::All => "tasks.tabs.all",
      Self::Running => "overview.tabs.running",
      Self::NeedsYou => "overview.tabs.needsYou",
      Self::Queued => "overview.tabs.queued",
    }
  }

  pub const fn includes(self, phase: TaskPhase) -> bool {
    match self {
      Self::All => true,
      Self::Running => matches!(phase, TaskPhase::Running),
      Self::NeedsYou => matches!(phase, TaskPhase::Waiting),
      Self::Queued => matches!(phase, TaskPhase::Queued),
    }
  }

  /// Tasks in the tab.
  pub fn count(self, tasks: &[Task], phases: &Phases) -> usize {
    tasks
      .iter()
      .filter(|task| self.includes(phase_of(task, phases)))
      .count()
  }

  /// The newest `limit` tasks of the tab, newest first.
  pub fn rows(self, tasks: &[Task], published: &[PipelineChoice], phases: &Phases, limit: usize) -> Vec<TaskRow> {
    tasks
      .iter()
      .rev()
      .filter(|task| self.includes(phase_of(task, phases)))
      .take(limit)
      .map(|task| TaskRow::new(task, published, phases.get(&task.id())))
      .collect()
  }
}

fn phase_of(task: &Task, phases: &Phases) -> TaskPhase {
  phases
    .get(&task.id())
    .map_or_else(|| TaskPhase::of(task, None), |latest| latest.phase)
}

/// The project pulse (`C95Z93`) counted from the tasks' latest runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pulse {
  /// Runs with an agent at work.
  pub running: usize,
  /// Runs with a step waiting for a person: a plan decision, a question or an approval.
  pub needs_input: usize,
  /// Runs that passed every step since the day began.
  pub completed_today: usize,
}

impl Pulse {
  /// `day_start` is local midnight of today.
  pub fn of(phases: &Phases, day_start: Timestamp) -> Self {
    phases
      .values()
      .fold(Self::default(), |pulse, latest| match latest.phase {
        TaskPhase::Running => Self {
          running: pulse.running + 1,
          ..pulse
        },
        TaskPhase::Waiting => Self {
          needs_input: pulse.needs_input + 1,
          ..pulse
        },
        TaskPhase::Succeeded if latest.updated_at >= day_start => Self {
          completed_today: pulse.completed_today + 1,
          ..pulse
        },
        _ => pulse,
      })
  }

  /// Under Running now: the agents at work while the runner is online (`None` until it
  /// reported), else the `lost` runs its absence interrupted ("2 runs interrupted" in `Z7Fyr8`).
  pub fn running_context(online: bool, agents_working: Option<usize>, lost: usize) -> Message {
    let count = |key_one: &'static str, key_other: &'static str, count: usize| {
      let key = if count == 1 { key_one } else { key_other };
      Message::new(key).with("count", count.to_string())
    };
    match (online, agents_working) {
      (true, Some(agents)) => count(
        "overview.metrics.agentsWorking.one",
        "overview.metrics.agentsWorking.other",
        agents,
      ),
      (true, None) => Message::new("overview.metrics.agentsUnknown"),
      (false, _) => count(
        "overview.metrics.interrupted.one",
        "overview.metrics.interrupted.other",
        lost,
      ),
    }
  }
}

/// A metric figure as the design draws it: at least two digits ("02", "12").
pub fn figure(count: usize) -> String {
  format!("{count:02}")
}

/// Under the overview's task list (`WV44u`): active tasks and completed ones.
pub fn footer(tasks: &[Task], phases: &Phases) -> Message {
  let (active, completed) =
    tasks
      .iter()
      .map(|task| phase_of(task, phases))
      .fold((0, 0), |(active, completed), phase| match phase {
        TaskPhase::Succeeded => (active, completed + 1),
        TaskPhase::Draft | TaskPhase::Failed | TaskPhase::Cancelled => (active, completed),
        _ => (active + 1, completed),
      });
  let key = if active == 1 {
    "overview.footer.one"
  } else {
    "overview.footer.other"
  };
  Message::new(key)
    .with("active", active.to_string())
    .with("completed", completed.to_string())
}

/// A task's latest run as an entry of Recent activity (`mBkeJ` in `YJ8PV`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentRun {
  pub task: TaskId,
  pub key: Arc<str>,
  pub number: RunNumber,
  pub phase: TaskPhase,
  /// When the run last changed.
  pub at: Timestamp,
}

impl RecentRun {
  /// "Run #128 completed".
  pub fn title(&self) -> Message {
    let key = match self.phase {
      TaskPhase::Draft | TaskPhase::Queued => "overview.activity.queued",
      TaskPhase::Running => "overview.activity.running",
      TaskPhase::Waiting => "overview.activity.waiting",
      TaskPhase::Paused => "overview.activity.paused",
      TaskPhase::StoppedAtLimit => "overview.activity.stoppedAtLimit",
      TaskPhase::Interrupted => "overview.activity.interrupted",
      TaskPhase::Succeeded => "overview.activity.succeeded",
      TaskPhase::Failed => "overview.activity.failed",
      TaskPhase::Cancelled => "overview.activity.cancelled",
    };
    Message::new(key).with("run", self.number.get().to_string())
  }
}

/// The `limit` most recently changed latest runs, newest first.
pub fn recent_runs(tasks: &[Task], phases: &Phases, limit: usize) -> Vec<RecentRun> {
  let mut recent: Vec<RecentRun> = tasks
    .iter()
    .filter_map(|task| {
      let latest = phases.get(&task.id())?;
      Some(RecentRun {
        task: task.id(),
        key: task.key().to_string().into(),
        number: latest.number,
        phase: latest.phase,
        at: latest.updated_at,
      })
    })
    .collect();
  recent.sort_by(|a, b| b.at.cmp(&a.at).then(b.number.cmp(&a.number)));
  recent.truncate(limit);
  recent
}
