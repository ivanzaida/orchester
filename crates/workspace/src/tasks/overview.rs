//! The project overview (02.08 `Twtqc`) as data: the project pulse (`C95Z93`), the task tabs
//! (`i2uFe`) with their rows and footer (`WV44u`), what needs the user (`EKUWS`) and the recent
//! run activity (`YJ8PV`). Everything is counted from the project's tasks and their latest runs.

use std::{num::NonZeroU32, sync::Arc};

use orchester_core::{
  id::TaskId,
  run::{RunNumber, TaskPhase},
  task::Task,
  time::Timestamp,
};

use super::{
  data::PipelineChoice,
  list::{Latest, Phases, TaskRow, phase_of},
};
use crate::messages::Message;

/// Tabs of the overview's task list (`i2uFe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewTab {
  All,
  Running,
  NeedsYou,
  Queued,
  Failed,
}

impl lurq::app::component::DevtoolsInspectable for OverviewTab {}

impl OverviewTab {
  pub const ALL: [Self; 5] = [Self::All, Self::Running, Self::NeedsYou, Self::Queued, Self::Failed];

  pub const fn label_key(self) -> &'static str {
    match self {
      Self::All => "tasks.tabs.all",
      Self::Running => "overview.tabs.running",
      Self::NeedsYou => "overview.tabs.needsYou",
      Self::Queued => "overview.tabs.queued",
      Self::Failed => "overview.tabs.failed",
    }
  }

  /// Whether a task in `phase` is in the tab; one whose latest run cannot be read (`None`) is only
  /// in All.
  pub const fn includes(self, phase: Option<TaskPhase>) -> bool {
    match self {
      Self::All => true,
      Self::Running => matches!(phase, Some(TaskPhase::Running)),
      Self::NeedsYou => matches!(phase, Some(phase) if phase.needs_you()),
      Self::Queued => matches!(phase, Some(TaskPhase::Queued)),
      Self::Failed => matches!(phase, Some(TaskPhase::Failed)),
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
  /// `day_start` is local midnight of today. A run that cannot be read counts nowhere.
  pub fn of(phases: &Phases, day_start: Timestamp) -> Self {
    phases
      .values()
      .filter_map(Latest::run)
      .fold(Self::default(), |pulse, latest| match latest.phase {
        TaskPhase::Running => Self {
          running: pulse.running + 1,
          ..pulse
        },
        TaskPhase::Waiting | TaskPhase::Stuck => Self {
          needs_input: pulse.needs_input + 1,
          ..pulse
        },
        TaskPhase::Succeeded | TaskPhase::Merged if latest.updated_at >= day_start => Self {
          completed_today: pulse.completed_today + 1,
          ..pulse
        },
        _ => pulse,
      })
  }

  /// Under Running now: the agents at work of the agent slots while the runner is online ("2 of 4
  /// agents working"; `None` until it reported), else the `lost` runs its absence interrupted ("2
  /// runs interrupted" in `Z7Fyr8`).
  pub fn running_context(online: bool, agents: Option<(usize, NonZeroU32)>, lost: usize) -> Message {
    match (online, agents) {
      (true, Some((working, slots))) => {
        let key = if slots.get() == 1 {
          "overview.metrics.agentsWorking.one"
        } else {
          "overview.metrics.agentsWorking.other"
        };
        Message::new(key)
          .with("count", working.to_string())
          .with("slots", slots.to_string())
      }
      (true, None) => Message::new("overview.metrics.agentsUnknown"),
      (false, _) => {
        let key = if lost == 1 {
          "overview.metrics.interrupted.one"
        } else {
          "overview.metrics.interrupted.other"
        };
        Message::new(key).with("count", lost.to_string())
      }
    }
  }
}

/// A count as every figure shows it (Overview metrics and headings, the status bar): the plain
/// number, "2", "12", "0", never zero-padded (decided 2026-09-27).
pub fn figure(count: usize) -> String {
  count.to_string()
}

/// Under the overview's task list (`WV44u`): active tasks and completed ones; a task whose latest
/// run cannot be read is neither.
pub fn footer(tasks: &[Task], phases: &Phases) -> Message {
  let (active, completed) =
    tasks
      .iter()
      .map(|task| phase_of(task, phases))
      .fold((0, 0), |(active, completed), phase| match phase {
        Some(TaskPhase::Succeeded | TaskPhase::Merged) => (active, completed + 1),
        Some(TaskPhase::Draft | TaskPhase::Failed | TaskPhase::Cancelled) | None => (active, completed),
        Some(_) => (active + 1, completed),
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
      TaskPhase::Waiting | TaskPhase::Stuck => "overview.activity.waiting",
      TaskPhase::Paused => "overview.activity.paused",
      TaskPhase::StoppedAtLimit => "overview.activity.stoppedAtLimit",
      TaskPhase::Interrupted => "overview.activity.interrupted",
      TaskPhase::Succeeded => "overview.activity.succeeded",
      TaskPhase::Merged => "overview.activity.merged",
      TaskPhase::Failed => "overview.activity.failed",
      TaskPhase::Cancelled => "overview.activity.cancelled",
    };
    Message::new(key).with("run", self.number.get().to_string())
  }
}

/// What Recent activity says when [`recent_runs`] lists nothing: that the project has no runs yet,
/// or, when tasks have runs whose latest cannot be read, that those runs cannot be listed.
pub fn activity_empty(phases: &Phases) -> Message {
  let unreadable = phases
    .values()
    .filter(|latest| matches!(latest, Latest::Unreadable(_)))
    .count();
  match unreadable {
    0 => Message::new("overview.activity.empty"),
    1 => Message::new("overview.activity.unreadable.one"),
    count => Message::new("overview.activity.unreadable.other").with("count", count.to_string()),
  }
}

/// The `limit` most recently changed latest runs, newest first. A run that cannot be read has no
/// known change to list; its task row shows it.
pub fn recent_runs(tasks: &[Task], phases: &Phases, limit: usize) -> Vec<RecentRun> {
  let mut recent: Vec<RecentRun> = tasks
    .iter()
    .filter_map(|task| {
      let latest = phases.get(&task.id())?.run()?;
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
