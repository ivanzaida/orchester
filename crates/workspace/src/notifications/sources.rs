//! The events of what the app follows, read again whenever it changes: one project's tasks as they stand with its
//! runner, its runner, its merges and its Orchestrator proposals ([`of_project`]), and the agent accounts' plan limits
//! ([`of_plan_limits`]). What waits for the person is what every *Needs you* counts ([`ProjectTasks::waiting`]), each
//! opening where the Needs you panel decides it ([`NeedItem::decided`]); the project's memory records are no case of
//! a notification. A run's end is read off its task's latest run, as the Orchestrator follows ends: merged in
//! Orchester or outside it, or failed. An event names only what a notification may show.

use std::sync::Arc;

use orchester_core::{
  connection::plan_limits::{LimitReading, LimitState, PlanReadings},
  id::{AgentAccountId, ProjectId, RunId},
  run::{RunLimit, TaskPhase},
  time::Timestamp,
};

use super::{Event, EventTask, Kind, LimitKind, Origin, Target, WaitingProposal};
use crate::{
  links::TaskPage,
  runner::state::{RunnerFailure, RunnerState},
  tasks::{
    attention::{Decided, NeedItem},
    data::ProjectTasks,
    integration_jobs::{JobState, ProjectJobState},
    list::Latest,
    needs_you::{WaitingItem, WaitingKind},
  },
};

/// How much of a plan window used is told: 90 %.
pub const PLAN_LIMIT_NEAR: f64 = 90.0;

/// The project a reading is of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
  pub id: ProjectId,
  pub name: Arc<str>,
}

/// A merge of a run that stopped where the person carries it on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeStop {
  pub run: RunId,
  /// At conflicts; otherwise where Local integration carries it on.
  pub conflicts: bool,
}

impl MergeStop {
  /// The merges of `jobs` that failed at conflicts or where Local integration carries on; other failures are
  /// told where they happened.
  pub fn of_jobs(jobs: &ProjectJobState) -> Vec<Self> {
    jobs
      .runs()
      .filter(|(_, state)| matches!(state.merge, JobState::Failed(_)))
      .filter_map(|(run, state)| {
        if state.continues_locally {
          Some(Self { run, conflicts: false })
        } else {
          (!state.conflicts.is_empty()).then_some(Self { run, conflicts: true })
        }
      })
      .collect()
  }
}

/// What one reading of a project holds.
pub struct ProjectReading<'a> {
  /// Its tasks as they stand with its runner.
  pub tasks: &'a ProjectTasks,
  /// Its runner; `None` while it has none in this session.
  pub runner: Option<&'a RunnerState>,
  pub merges: &'a [MergeStop],
  /// The runs its Orchestrator chat merged by itself ([`super::ChatReading::merged`]): their merge is no event.
  pub chat_merges: &'a [RunId],
}

/// Every event standing in `project` as `reading` holds it, but its Orchestrator proposals ([`of_proposals`]).
pub fn of_project(project: &Project, reading: &ProjectReading) -> Vec<Event> {
  let mut events: Vec<Event> = reading
    .tasks
    .waiting()
    .iter()
    .filter_map(|item| waiting(project, reading.tasks, item))
    .collect();
  events.extend(ended(project, reading.tasks, reading.chat_merges));
  events.extend(reading.runner.and_then(|state| runner(project, state)));
  events.extend(
    reading
      .merges
      .iter()
      .filter_map(|stop| merge(project, reading.tasks, *stop)),
  );
  events
}

/// The events of `project`'s Orchestrator proposals that wait for the person. They are read with its conversations,
/// apart from its tasks, so they are observed in a scope of their own (`Scope::Chats`).
pub fn of_proposals(project: &Project, proposals: &[WaitingProposal]) -> Vec<Event> {
  proposals
    .iter()
    .map(|proposal| Event {
      key: format!("proposal:{}:{}", proposal.chat, proposal.seq).into(),
      batch: None,
      kind: Kind::Proposal,
      origin: origin(project),
      task: None,
      step: None,
      target: Target::Orchestrator(project.id, proposal.chat),
    })
    .collect()
}

fn origin(project: &Project) -> Origin {
  Origin::Project {
    id: project.id,
    name: project.name.clone(),
  }
}

/// The event of `item`, one of what waits in `tasks`; none for the project's memory records.
fn waiting(project: &Project, tasks: &ProjectTasks, item: &WaitingItem) -> Option<Event> {
  let run = item.run.map_or_else(String::new, |(run, _)| run.to_string());
  let step = item.step.as_ref().map_or("", |step| &step.name);
  let (kind, key) = match &item.kind {
    // What the merge queue waits for, and a run that committed nothing (ORC-93), show in Needs you and on their
    // screens; they raise no notification yet.
    WaitingKind::Memory { .. }
    | WaitingKind::DeliveryLeft(_)
    | WaitingKind::DeliveryBlocked { .. }
    | WaitingKind::ReadyToComplete
    | WaitingKind::CommittedNothing => return None,
    WaitingKind::Approval(shown) => (
      Kind::Approval {
        command: shown.approval.command,
      },
      format!("approval:{}", shown.approval.id),
    ),
    WaitingKind::Choice(choice) => (
      Kind::Question {
        agent: choice.agent.clone(),
      },
      format!("choice:{run}:{}", choice.request),
    ),
    WaitingKind::Question(Some(asked)) => (
      Kind::Question {
        agent: asked.agent.clone(),
      },
      format!("question:{run}:{step}"),
    ),
    WaitingKind::Question(None) => (Kind::Message, format!("message:{run}:{step}")),
    WaitingKind::Plan(plan) => (
      Kind::Plan {
        version: plan.version.get(),
      },
      format!("plan:{run}:{step}:{}", plan.version),
    ),
    WaitingKind::Decision(decision) => (
      Kind::Decision(decision.why),
      format!("decision:{run}:{}:{:?}", decision.step, decision.why),
    ),
    WaitingKind::Interrupted { .. } => (Kind::Interrupted, format!("interrupted:{run}")),
    WaitingKind::StoppedAtLimit { .. } => (Kind::Limit(limit_of(tasks, item)), format!("limit:{run}")),
    WaitingKind::ReadyToMerge => (Kind::ReadyToMerge, format!("ready:{run}")),
  };
  let batch = batch(project, &kind, item.since);
  let task = item.task.as_ref().and_then(|task| {
    Some(EventTask {
      key: task.key.parse().ok()?,
      title: task.title.clone(),
    })
  });
  let target = match (&task, NeedItem::of(tasks, item.clone()).decided()) {
    (Some(task), Decided::Card(_) | Decided::Conversation) => task_target(project, task, TaskPage::Conversation, None),
    (Some(task), Decided::Summary(card)) => task_target(project, task, TaskPage::Summary, card),
    (_, Decided::Unavailable(_) | Decided::Memory | Decided::MergeQueue) | (None, _) => Target::Runner(project.id),
  };
  Some(Event {
    key: key.into(),
    batch,
    kind,
    origin: origin(project),
    task,
    step: item.step.as_ref().map(|step| step.name.clone()),
    target,
  })
}

fn task_target(
  project: &Project,
  task: &EventTask,
  page: TaskPage,
  card: Option<crate::tasks::attention::SummaryCard>,
) -> Target {
  Target::Task {
    project: project.id,
    key: task.key.clone(),
    page,
    card,
  }
}

/// The limit `item`'s run stopped at, as its task's latest run says.
fn limit_of(tasks: &ProjectTasks, item: &WaitingItem) -> Option<LimitKind> {
  let task = item.task.as_ref()?;
  let limit = tasks.phases.get(&task.id)?.run()?.limit?;
  Some(match limit {
    RunLimit::Tokens => LimitKind::Tokens,
    RunLimit::Time => LimitKind::Time,
    RunLimit::DailyBudget => LimitKind::DailyBudget,
    RunLimit::GroupBudget => LimitKind::GroupBudget,
    RunLimit::StepBudget { .. } => LimitKind::StepBudget,
    RunLimit::PlanLimit { .. } => LimitKind::PlanLimit,
  })
}

/// The batch of a stop at a budget the project's runs share: those it stopped the same day are one notification.
fn batch(project: &Project, kind: &Kind, since: Timestamp) -> Option<Arc<str>> {
  let shared = match kind {
    Kind::Limit(Some(LimitKind::DailyBudget)) => "daily",
    Kind::Limit(Some(LimitKind::GroupBudget)) => "group",
    _ => return None,
  };
  let day = since.local_day_start().unix_millis();
  Some(format!("limit:{shared}:{}:{day}", project.id).into())
}

/// The ends of the tasks' latest runs that are no wait: merged, by a person in Orchester or outside it, and failed.
/// A merge the Orchestrator's chat carried out itself, one of `chat_merges`, is its own action and no event.
fn ended(project: &Project, tasks: &ProjectTasks, chat_merges: &[RunId]) -> Vec<Event> {
  tasks
    .tasks
    .iter()
    .filter_map(|task| {
      let Some(Latest::Run(latest)) = tasks.phases.get(&task.id()) else {
        return None;
      };
      let (kind, name) = match latest.phase {
        TaskPhase::Merged if chat_merges.contains(&latest.id) => return None,
        TaskPhase::Merged => (Kind::Merged, "merged"),
        TaskPhase::Failed => (Kind::Failed, "failed"),
        _ => return None,
      };
      let task = EventTask {
        key: task.key().clone(),
        title: task.details().title.as_str().into(),
      };
      Some(Event {
        key: format!("{name}:{}", latest.id).into(),
        batch: None,
        kind,
        origin: origin(project),
        target: task_target(project, &task, TaskPage::Summary, None),
        task: Some(task),
        step: latest.step.as_ref().map(|step| step.name.clone()),
      })
    })
    .collect()
}

/// The project's runner, while it went away and did not come back, or could not start. One that reconnects or was
/// stopped is no event.
fn runner(project: &Project, state: &RunnerState) -> Option<Event> {
  let RunnerState::Failed { cause, attempts } = state else {
    return None;
  };
  let (kind, key) = if *attempts == 0 && *cause != RunnerFailure::Lost {
    (Kind::RunnerNotStarted, "runner:not-started")
  } else {
    (Kind::RunnerOffline, "runner:offline")
  };
  Some(Event {
    key: key.into(),
    batch: None,
    kind,
    origin: origin(project),
    task: None,
    step: None,
    target: Target::Runner(project.id),
  })
}

/// The merge `stop` of a task's latest run of `tasks`; none for an earlier run's.
fn merge(project: &Project, tasks: &ProjectTasks, stop: MergeStop) -> Option<Event> {
  let task = tasks.tasks.iter().find(|task| {
    tasks
      .phases
      .get(&task.id())
      .and_then(Latest::run)
      .is_some_and(|latest| latest.id == stop.run)
  })?;
  let task = EventTask {
    key: task.key().clone(),
    title: task.details().title.as_str().into(),
  };
  let (kind, page, name) = if stop.conflicts {
    (Kind::MergeConflicts, TaskPage::Summary, "conflicts")
  } else {
    (Kind::LocalIntegration, TaskPage::LocalIntegration, "local")
  };
  Some(Event {
    key: format!("merge:{}:{name}", stop.run).into(),
    batch: None,
    kind,
    origin: origin(project),
    target: task_target(project, &task, page, None),
    task: Some(task),
    step: None,
  })
}

/// Every account's plan window used [`PLAN_LIMIT_NEAR`] or more, or reached, and every such window since reset, as
/// `readings` read at `now` hold them; `name` names an account as Settings › Accounts does.
pub fn of_plan_limits(readings: &PlanReadings, now: Timestamp, name: impl Fn(&LimitReading) -> Arc<str>) -> Vec<Event> {
  let mut events = Vec::new();
  for reading in readings.all() {
    let Some(account) = reading.account_id() else {
      continue;
    };
    for window in &reading.limits.windows {
      let reached = reading.window_state(window, now) == LimitState::Reached;
      let percent = window.percent().unwrap_or(if reached { 100.0 } else { 0.0 });
      if percent < PLAN_LIMIT_NEAR && !reached {
        continue;
      }
      let resets = window.resets().map_or(0, Timestamp::unix_millis);
      let (kind, key) = if window.expired(now) {
        (Kind::PlanLimitReset, "plan-reset")
      } else {
        (
          Kind::PlanLimitNear {
            percent: whole(percent),
          },
          "plan-near",
        )
      };
      events.push(account_event(
        (account, name(reading)),
        kind,
        format!("{key}:{account}:{}:{resets}", window.minutes()),
      ));
    }
  }
  events
}

fn account_event((account, name): (AgentAccountId, Arc<str>), kind: Kind, key: String) -> Event {
  Event {
    key: key.into(),
    batch: None,
    kind,
    origin: Origin::Account { name },
    task: None,
    step: None,
    target: Target::Account(account),
  }
}

/// A percentage of a window as a whole number, at most 100.
fn whole(percent: f64) -> u32 {
  // Clamped to 0..=100 first, so the cast keeps the whole part and never wraps.
  percent.clamp(0.0, 100.0).floor() as u32
}
