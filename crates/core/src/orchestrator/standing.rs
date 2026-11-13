//! Where a target a conversation leads stands (`docs/plan/orchestrator-autonomy/OA-12-…`): one rule that says, for
//! each of its tasks, whether it is done, works, waits and for what, can start, is blocked, failed or is left to the
//! person ([`TaskStanding`]), and for the whole whether work can start, something still moves, nothing can move any
//! more, or all is done ([`TargetStanding`]). The app raises its events from it and a leading turn reads it first. A
//! pure rule: what it reads of each task is [`TaskFacts`], read from the project by its caller.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{Ceilings, WatchTarget};
use crate::run::TaskPhase;

/// Who left a task to the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parked {
  /// A person marked it Blocked.
  ByPerson,
  /// The Orchestrator parked it for the person.
  ByChat,
}

/// Why a task cannot be started whatever else holds, which only the person changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unready {
  NoPipeline,
  PipelineNotReady,
}

/// What the rule reads of one task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskFacts {
  /// Its key, as people read it.
  pub key: String,
  /// Its workflow status is Done or Canceled.
  pub closed: bool,
  /// Where its latest run stands; `Draft` or `Queued` for a task with no run.
  pub phase: TaskPhase,
  /// How many runs it had.
  pub runs: usize,
  pub unready: Option<Unready>,
  pub parked: Option<Parked>,
  /// The keys of the unfinished tasks that block it.
  pub blockers: Vec<String>,
  /// Its paused run waits for disk space, and goes on by itself.
  pub low_disk: bool,
  /// New starts of it are held, in the project or in its group.
  pub held: bool,
}

/// Where one task stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskStanding {
  /// Done or Canceled, or its work is merged.
  Done,
  /// An agent works on it, it is queued to start, or its interrupted run is continued.
  Working,
  /// A step or the run waits for an answer or a decision.
  WaitsForPerson,
  /// Its run stopped at a limit: a raise, or the reset of the daily budget, moves it.
  WaitsForLimit,
  /// Its run is paused until disk space is free again.
  WaitsForDisk,
  /// A person paused its run.
  Paused,
  /// Its run succeeded and waits to be merged.
  ToMerge,
  /// Nothing keeps it from being started.
  ReadyToStart,
  /// New starts of it are held.
  Held,
  /// Unfinished tasks block it, by their keys.
  BlockedBy(Vec<String>),
  /// Its latest run failed after `runs` runs; `again`: the ceilings of what leads it allow another.
  Failed {
    runs: usize,
    again: bool,
  },
  /// Its latest run was canceled, and the task is open.
  Canceled,
  Parked(Parked),
  NotReady(Unready),
}

impl TaskFacts {
  /// Where the task stands, `ceilings` being those of the subscription that leads it to the end (`None`: it is only
  /// followed, or the task is outside what is asked about, and nothing runs it again).
  pub fn standing(&self, ceilings: Option<Ceilings>) -> TaskStanding {
    if self.closed || self.phase == TaskPhase::Merged {
      return TaskStanding::Done;
    }
    if let Some(parked) = self.parked {
      return TaskStanding::Parked(parked);
    }
    match self.phase {
      TaskPhase::Running | TaskPhase::Interrupted => TaskStanding::Working,
      TaskPhase::Waiting | TaskPhase::Stuck => TaskStanding::WaitsForPerson,
      TaskPhase::StoppedAtLimit => TaskStanding::WaitsForLimit,
      TaskPhase::Paused if self.low_disk => TaskStanding::WaitsForDisk,
      TaskPhase::Paused => TaskStanding::Paused,
      TaskPhase::Succeeded => TaskStanding::ToMerge,
      TaskPhase::Merged => TaskStanding::Done,
      TaskPhase::Failed => TaskStanding::Failed {
        runs: self.runs,
        again: ceilings.is_some_and(|ceilings| ceilings.another_run(self.runs).is_ok()),
      },
      TaskPhase::Cancelled => TaskStanding::Canceled,
      TaskPhase::Draft => self.start().unwrap_or(TaskStanding::ReadyToStart),
      TaskPhase::Queued => self.start().unwrap_or(TaskStanding::Working),
    }
  }

  /// What keeps a task with no run from starting; `None` when nothing does.
  fn start(&self) -> Option<TaskStanding> {
    if let Some(unready) = self.unready {
      return Some(TaskStanding::NotReady(unready));
    }
    if !self.blockers.is_empty() {
      return Some(TaskStanding::BlockedBy(self.blockers.clone()));
    }
    self.held.then_some(TaskStanding::Held)
  }
}

impl TaskStanding {
  /// The name it is told by.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Done => "done",
      Self::Working => "working",
      Self::WaitsForPerson => "waits_for_person",
      Self::WaitsForLimit => "waits_for_limit",
      Self::WaitsForDisk => "waits_for_disk",
      Self::Paused => "paused",
      Self::ToMerge => "to_merge",
      Self::ReadyToStart => "ready_to_start",
      Self::Held => "held",
      Self::BlockedBy(_) => "blocked_by",
      Self::Failed { .. } => "failed",
      Self::Canceled => "canceled",
      Self::Parked(Parked::ByPerson) => "parked_by_person",
      Self::Parked(Parked::ByChat) => "parked",
      Self::NotReady(Unready::NoPipeline) => "no_pipeline",
      Self::NotReady(Unready::PipelineNotReady) => "pipeline_not_ready",
    }
  }

  /// Whether it goes on without anyone starting anything: an agent works, an event of it is the chat's to take up (a
  /// wait, a limit, a success to merge), or it goes on by itself.
  const fn moves(&self) -> bool {
    matches!(
      self,
      Self::Working | Self::WaitsForPerson | Self::WaitsForLimit | Self::WaitsForDisk | Self::ToMerge
    )
  }

  /// Whether the chat that leads it may start it now: it never ran, or it failed within its ceiling of runs.
  const fn starts(&self) -> bool {
    matches!(self, Self::ReadyToStart | Self::Failed { again: true, .. })
  }
}

/// What the rule reads around the tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Around {
  /// How many more agents may work now.
  pub free_slots: usize,
  /// The ceilings of the subscription that leads the target to the end; `None` while it only follows.
  pub ceilings: Option<Ceilings>,
  /// The subscription's budget is spent: nothing more is started under it.
  pub budget_used: bool,
}

/// Where the whole target stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetStanding {
  /// Every task is done; a target with no task is.
  Done,
  /// These tasks can be started now, by their keys; `slot_free`: an agent may start at once.
  CanStart { tasks: Vec<String>, slot_free: bool },
  /// Nothing can be started, and something still moves.
  Working,
  /// Nothing can be started and nothing moves: what is left waits for the person. `budget_used`: tasks could start
  /// but for the subscription's budget.
  Stuck { budget_used: bool },
}

impl TargetStanding {
  /// The name it is told by.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Done => "done",
      Self::CanStart { .. } => "can_start",
      Self::Working => "working",
      Self::Stuck { .. } => "stuck",
    }
  }
}

/// Where a target and each of its tasks stand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
  /// The target's tasks, in the order they were given, by their keys.
  pub tasks: Vec<(String, TaskStanding)>,
  pub target: TargetStanding,
}

/// Where the target of tasks `target` stands. `outside`: the tasks outside it that block some of its tasks; they are
/// read to tell whether what waits on them can still move, and never started from here. A blocking task that is in
/// neither list is taken as one that will not move.
pub fn standing(target: &[TaskFacts], outside: &[TaskFacts], around: &Around) -> Standing {
  let tasks: Vec<(String, TaskStanding)> = target
    .iter()
    .map(|task| (task.key.clone(), task.standing(around.ceilings)))
    .collect();
  let startable: Vec<String> = tasks
    .iter()
    .filter(|(_, standing)| standing.starts())
    .map(|(key, _)| key.clone())
    .collect();
  let target = if tasks.iter().all(|(_, standing)| *standing == TaskStanding::Done) {
    TargetStanding::Done
  } else if !startable.is_empty() && !around.budget_used {
    TargetStanding::CanStart {
      tasks: startable,
      slot_free: around.free_slots > 0,
    }
  } else if moving(&tasks, outside) {
    TargetStanding::Working
  } else {
    TargetStanding::Stuck {
      budget_used: around.budget_used && !startable.is_empty(),
    }
  };
  Standing { tasks, target }
}

/// Whether something of `tasks` still moves: a task that moves by itself, or one blocked only by tasks that do, in the
/// target or `outside` it, however long the chain.
fn moving(tasks: &[(String, TaskStanding)], outside: &[TaskFacts]) -> bool {
  let outside: Vec<(String, TaskStanding)> = outside
    .iter()
    .map(|task| (task.key.clone(), task.standing(None)))
    .collect();
  let all: HashMap<&str, &TaskStanding> = tasks
    .iter()
    .chain(&outside)
    .map(|(key, standing)| (key.as_str(), standing))
    .collect();
  let mut alive: HashSet<&str> = all
    .iter()
    .filter(|(_, standing)| standing.moves())
    .map(|(key, _)| *key)
    .collect();
  // A task blocked only by tasks that move will move after them; each pass adds those one link further.
  loop {
    let more: Vec<&str> = all
      .iter()
      .filter(|(key, _)| !alive.contains(*key))
      .filter(|(_, standing)| {
        matches!(standing, TaskStanding::BlockedBy(by) if by.iter().all(|blocker| alive.contains(blocker.as_str())))
      })
      .map(|(key, _)| *key)
      .collect();
    if more.is_empty() {
      break;
    }
    alive.extend(more);
  }
  tasks.iter().any(|(key, _)| alive.contains(key.as_str()))
}

/// What a leading subscription's standing raises in its conversation, once per state
/// (`docs/plan/orchestrator-autonomy/OA-13-…`, `OA-15-…`): work can start, or the target is stuck.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "raised", rename_all = "snake_case")]
pub enum Raised {
  /// These tasks can be started now and an agent is free, by their keys.
  CanStart { tasks: Vec<String> },
  /// Nothing can start and nothing moves: what is left, each with where it stands.
  Stuck {
    left: Vec<Left>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    budget_used: bool,
  },
}

/// A task left when a target is stuck: its key, where it stands ([`TaskStanding::name`]) and what that waits for,
/// when something tells (the tasks that block it, or what the chat asked of the person when it parked it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Left {
  pub task: String,
  pub standing: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub note: Option<String>,
}

impl Raised {
  /// What `standing` raises: work that can start while an agent is free, or a stuck target with what is left of it,
  /// each task's note read from `note` when the standing itself tells none. `None` while the target works, is done,
  /// or could start work but for a free agent.
  pub fn of(standing: &Standing, note: impl Fn(&str) -> Option<String>) -> Option<Self> {
    match &standing.target {
      TargetStanding::CanStart { tasks, slot_free: true } => Some(Self::CanStart { tasks: tasks.clone() }),
      TargetStanding::Stuck { budget_used } => Some(Self::Stuck {
        left: standing
          .tasks
          .iter()
          .filter(|(_, standing)| *standing != TaskStanding::Done)
          .map(|(task, standing)| Left {
            task: task.clone(),
            standing: standing.name().to_owned(),
            note: match standing {
              TaskStanding::BlockedBy(by) => Some(by.join(", ")),
              _ => note(task),
            },
          })
          .collect(),
        budget_used: *budget_used,
      }),
      TargetStanding::CanStart { slot_free: false, .. } | TargetStanding::Working | TargetStanding::Done => None,
    }
  }

  /// What tells one state from another: an event is raised when it changes, never twice for the same.
  pub fn state(&self) -> String {
    match self {
      Self::CanStart { tasks } => format!("can_start:{}", tasks.join(",")),
      Self::Stuck { left, budget_used } => {
        let left: Vec<String> = left
          .iter()
          .map(|left| format!("{}={}", left.task, left.standing))
          .collect();
        format!("stuck:{}:{budget_used}", left.join(","))
      }
    }
  }
}

/// The state kept while a standing raises nothing ([`Raised::state`] otherwise).
pub const QUIET: &str = "quiet";

/// An event of a target a conversation leads to the end, as the conversation keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandingEvent {
  pub target: WatchTarget,
  /// The module's name, or the task's key as people read it; `None` for the tasks without a module and the whole
  /// project.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(flatten)]
  pub raised: Raised,
  /// It starts an Orchestrator turn by itself: *Let the Orchestrator drive* was on when it was kept.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub automatic: bool,
}
