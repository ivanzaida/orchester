//! The project overview (02.08 `Twtqc`) as data: the project pulse (`C95Z93`), the task tabs
//! (`i2uFe`) with their rows and footer (`WV44u`), what needs the user (`EKUWS`) and the recent
//! run activity (`YJ8PV`). Everything is counted from the project's tasks and their latest runs.

use std::{num::NonZeroU32, sync::Arc};

use orchester_core::{
  id::TaskId,
  merge_queue::{Left, QueueState},
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
  /// Where it stands in the project's merge queue, once it passed every step and joined it ([`recent_activity`]),
  /// with its place among the waiting runs.
  pub queue: Option<(QueueState, Option<usize>)>,
}

impl RecentRun {
  /// "Run #128 completed"; a run of the merge queue as the queue has it: *Run #134 joined the merge queue · 1st in
  /// line*, *Run #133 is ready to merge*, *Run #128 left the merge queue: Integrate couldn't resolve 2 files*.
  pub fn title(&self) -> Message {
    if self.phase == TaskPhase::Succeeded
      && let Some((state, place)) = &self.queue
      && let Some(title) = queue_title(state, *place)
    {
      return title.with("run", self.number.get().to_string());
    }
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

/// What a run in `state` of the merge queue did last, in Recent activity's words; `None` once a person took it out.
fn queue_title(state: &QueueState, place: Option<usize>) -> Option<Message> {
  Some(match state {
    QueueState::Queued => Message::new("overview.activity.queue.joined").with("place", place.unwrap_or(1).to_string()),
    QueueState::Syncing | QueueState::Rechecking { .. } => Message::new("overview.activity.queue.delivering"),
    QueueState::Integrating { files, .. } => {
      Message::new("overview.activity.queue.integrating").with("count", files.len().to_string())
    }
    QueueState::WaitingForApproval => Message::new("overview.activity.queue.ready"),
    QueueState::Blocked(_) => Message::new("overview.activity.queue.blocked"),
    QueueState::Left(Left::Conflicts { files }) => {
      Message::new("overview.activity.queue.conflicts").with("count", files.len().to_string())
    }
    QueueState::Left(Left::KeepsMoving { base, .. }) => {
      Message::new("overview.activity.queue.keepsMoving").with("base", base.clone())
    }
    QueueState::Left(Left::CheckFailed { tool, .. }) => {
      Message::new("overview.activity.queue.checkFailed").with("tool", tool.clone())
    }
    QueueState::Left(Left::Stopped { .. } | Left::LaterRun) => Message::new("overview.activity.queue.stopped"),
    QueueState::Merged(_) | QueueState::Removed => return None,
  })
}

/// Recent activity of `tasks` ([`recent_runs`]), each run of the merge queue as the queue has it.
pub fn recent_activity(tasks: &super::data::ProjectTasks, limit: usize) -> Vec<RecentRun> {
  let waiting: Vec<_> = tasks
    .merge_queue
    .iter()
    .filter(|entry| entry.state == QueueState::Queued)
    .map(|entry| entry.run)
    .collect();
  let mut recent = recent_runs(&tasks.tasks, &tasks.phases, usize::MAX);
  for run in &mut recent {
    let Some(latest) = tasks.phases.get(&run.task).and_then(Latest::run) else {
      continue;
    };
    let Some(entry) = tasks.merge_queue.iter().find(|entry| entry.run == latest.id) else {
      continue;
    };
    let place = waiting.iter().position(|queued| *queued == entry.run).map(|index| index + 1);
    // The queue's own changes are when the run last changed for the person.
    run.at = run.at.max(entry.since);
    run.queue = Some((entry.state.clone(), place));
  }
  recent.sort_by(|a, b| b.at.cmp(&a.at).then(b.number.cmp(&a.number)));
  recent.truncate(limit);
  recent
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
        queue: None,
      })
    })
    .collect();
  recent.sort_by(|a, b| b.at.cmp(&a.at).then(b.number.cmp(&a.number)));
  recent.truncate(limit);
  recent
}
