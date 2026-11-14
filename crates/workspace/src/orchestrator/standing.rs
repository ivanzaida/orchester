//! Where what a conversation watches stands (`orchester_core::orchestrator::standing`,
//! `docs/plan/orchestrator-autonomy/OA-12-…`): the facts the rule reads, read once from the project database for
//! every subscription, and for a subscription that leads to the end the events its standing raises in its
//! conversation, once per state: work can start (OA-13), or the target is stuck (OA-15). A local and a team project
//! read the same way: a team task is the one its runs recorded, its module and whether it is closed being the team
//! server's last reading when it saw the task ([`super::team_status`]).

use std::{
  collections::{HashMap, HashSet},
  path::Path,
};

use orchester_core::{
  id::{ModuleId, ResourceId, TaskId},
  module::ModuleState,
  orchestrator::{
    ChatEntry, EntryKind, Subscription, Subscriptions, WatchTarget,
    parking::Parking,
    standing::{Around, Parked, QUIET, Raised, Standing, StandingEvent, TaskFacts, Unready, standing},
  },
  run::{PauseReason, RunStatus, TaskPhase},
  store::{MergeStore, ResourceStore, SettingsStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::{
  ceilings::spent_among,
  team_status::{TeamReading, reading_of},
};
use crate::{
  messages::{self, Message},
  tasks::{
    data::published_pipelines,
    hold,
    recorded::{self, WithRuns, is_team},
  },
};

/// What the standing of every subscription is read from, read once.
pub struct Reads {
  tasks: Vec<WithRuns>,
  facts: HashMap<TaskId, TaskFacts>,
  parked: HashMap<TaskId, Parking>,
  team: TeamReading,
  /// The team tasks the team server's reading saw that never ran on this computer, with the module each is in: no
  /// record here defines them, and the conversation that leads their module has them still to start.
  unrun: Vec<(TaskId, Option<ModuleId>)>,
  subscriptions: Subscriptions,
  /// How many more agents may work now.
  free_slots: usize,
}

/// A subscription with where its target stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Led {
  pub subscription: Subscription,
  /// The module's name or the task's key; `None` for the tasks without a module and the whole project.
  pub name: Option<String>,
  pub standing: Standing,
  /// The parkings of the target's tasks the chat parked, by their keys.
  pub parked: Vec<(String, Parking)>,
}

impl Reads {
  /// Reads the project of `db`. Blocks on SQLite.
  pub fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    let tasks = recorded::all_with_runs(db)?;
    let team = reading_of(db);
    let parked: HashMap<TaskId, Parking> = db
      .parked_tasks()?
      .into_iter()
      .map(|parking| (parking.task, parking))
      .collect();
    let published: HashSet<ResourceId> = published_pipelines(db)?.into_iter().map(|choice| choice.id).collect();
    let keys: HashMap<TaskId, String> = tasks
      .iter()
      .map(|(task, _)| (task.id(), task.key().to_string()))
      .collect();
    let holds = db.queue_holds()?;
    let acceptances = db.acceptance_task_ids()?;
    let mut facts = HashMap::new();
    let mut running = 0;
    for (task, runs) in &tasks {
      let held = holds.project || hold::group_of(db, task.id())?.is_some_and(|group| holds.groups.contains(&group));
      let blockers = db.unfinished_blocking_tasks(task.id())?;
      let read = Around1 {
        db,
        team: &team,
        published: &published,
        parked: parked.contains_key(&task.id()),
        acceptance: acceptances.contains(&task.id()),
      };
      let of_task = read.facts(
        task,
        runs,
        (held, blockers.iter().filter_map(|id| keys.get(id).cloned()).collect()),
      )?;
      running += usize::from(of_task.phase == TaskPhase::Running);
      facts.insert(task.id(), of_task);
    }
    let mut unrun = Vec::new();
    for (id, seen) in team.seen() {
      if facts.contains_key(&id) {
        continue;
      }
      facts.insert(
        id,
        TaskFacts {
          key: seen.key.to_string(),
          closed: seen.status.is_closed(),
          phase: TaskPhase::Draft,
          runs: 0,
          // A team task's pipeline is the team's, whose readiness the server checks when the work starts.
          unready: None,
          parked: if parked.contains_key(&id) {
            Some(Parked::ByChat)
          } else {
            seen.blocked.as_ref().map(|_| Parked::ByPerson)
          },
          blockers: Vec::new(),
          low_disk: false,
          held: holds.project,
        },
      );
      unrun.push((id, seen.module));
    }
    let slots = usize::try_from(db.project_limits()?.agent_slots().get().get()).unwrap_or(usize::MAX);
    Ok(Self {
      tasks,
      facts,
      parked,
      unrun,
      team,
      subscriptions: db.active_subscriptions()?,
      free_slots: slots.saturating_sub(running),
    })
  }

  /// Where what `subscription` leads stands: its tasks are those whose events go to it, the most specific
  /// subscription that covers each.
  pub fn led(&self, db: &ProjectDb, subscription: &Subscription) -> Result<Led, StoreError> {
    let leads = |id: TaskId, module: Option<ModuleId>| self.subscriptions.leader_of(id, module) == Some(subscription);
    let ran = self
      .tasks
      .iter()
      .map(|(task, _)| (task.id(), self.team.module_of(task)));
    let inside: Vec<TaskId> = ran
      .chain(self.unrun.iter().copied())
      .filter(|(id, module)| leads(*id, *module))
      .map(|(id, _)| id)
      .collect();
    let ids: HashSet<TaskId> = inside.iter().copied().collect();
    let target: Vec<TaskFacts> = inside.iter().filter_map(|id| self.facts.get(id).cloned()).collect();
    let blocking: HashSet<&str> = target
      .iter()
      .flat_map(|facts| facts.blockers.iter().map(String::as_str))
      .collect();
    let outside: Vec<TaskFacts> = self
      .facts
      .iter()
      .filter(|(id, facts)| !ids.contains(id) && blocking.contains(facts.key.as_str()))
      .map(|(_, facts)| facts.clone())
      .collect();
    let ceilings = subscription.mode().ceilings();
    let budget_used = match ceilings {
      Some(ceilings) if ceilings.budget().is_some() => ceilings
        .within_budget(spent_among(db, &self.tasks, (&self.subscriptions, subscription))?)
        .is_err(),
      Some(_) | None => false,
    };
    // A module with a branch of its own is not done until that branch is on its base branch.
    let delivery = match subscription.target() {
      WatchTarget::Module(id) => match db.module(id)? {
        Some(module) => Some(crate::modules::delivery::facts(db, &module)?.step()),
        None => None,
      },
      WatchTarget::Task(_) | WatchTarget::WithoutModule | WatchTarget::Project => None,
    };
    let around = Around {
      free_slots: self.free_slots,
      ceilings,
      budget_used,
      delivery,
    };
    let parked = inside
      .iter()
      .filter_map(|id| Some((self.facts.get(id)?.key.clone(), self.parked.get(id)?.clone())))
      .collect();
    Ok(Led {
      subscription: subscription.clone(),
      name: target_name(db, subscription.target())?,
      standing: standing(&target, &outside, &around),
      parked,
    })
  }

  /// Every active subscription with where its target stands, in the order they were kept.
  pub fn all(&self, db: &ProjectDb) -> Result<Vec<Led>, StoreError> {
    self
      .subscriptions
      .as_slice()
      .iter()
      .map(|subscription| self.led(db, subscription))
      .collect()
  }
}

/// What one task's facts are read with.
struct Around1<'a> {
  db: &'a ProjectDb,
  team: &'a TeamReading,
  published: &'a HashSet<ResourceId>,
  parked: bool,
  /// The task is a module's acceptance, which Orchester made.
  acceptance: bool,
}

impl Around1<'_> {
  fn facts(
    &self,
    task: &Task,
    runs: &WithRunsOf,
    (held, blockers): (bool, Vec<String>),
  ) -> Result<TaskFacts, StoreError> {
    let latest = runs.iter().rev().find_map(|listed| listed.as_ref().ok());
    let merge = match latest {
      Some(run) => self.db.run_merge(run.id())?,
      None => None,
    };
    // A latest run that cannot be read is the person's to look at: it reads as waiting for them.
    let phase = match (latest, runs.last()) {
      (None, Some(_)) => TaskPhase::Stuck,
      _ => TaskPhase::of(task, latest, merge.as_ref()),
    };
    let team = is_team(self.db, task.id())?;
    // A team task is closed when the team server last said so; this computer keeps no status of it to trust.
    let closed = match self.team.status_of(task.id()) {
      Some(status) => status.is_closed(),
      None => !team && task.organization().status().is_closed(),
    };
    // An acceptance that did not pass is done as an attempt: its findings become tasks, and it is not run again.
    let closed = closed || (self.acceptance && matches!(phase, TaskPhase::Failed | TaskPhase::Cancelled));
    // A team task's pipeline is the team's, whose readiness the server checks when the work starts.
    let unready = match task.details().pipeline {
      _ if team => None,
      None => Some(Unready::NoPipeline),
      Some(pipeline) if !self.published.contains(&pipeline) => Some(Unready::PipelineNotReady),
      Some(_) => None,
    };
    // A team task's Blocked mark is the team server's: the last reading holds it, this computer's records never do.
    let blocked = task.organization().blocked_reason().is_some()
      || self.team.task(task.id()).is_some_and(|seen| seen.blocked.is_some());
    let parked = if self.parked {
      Some(Parked::ByChat)
    } else {
      blocked.then_some(Parked::ByPerson)
    };
    Ok(TaskFacts {
      key: task.key().to_string(),
      closed,
      phase,
      runs: runs.len(),
      unready,
      parked,
      blockers,
      low_disk: latest.is_some_and(|run| *run.status() == RunStatus::Paused(PauseReason::LowDisk)),
      held,
    })
  }
}

type WithRunsOf = [orchester_core::run::ListedRun<orchester_store::RunStoreError>];

/// The name of `target` as people read it: an open module's name or a task's key; `None` for the tasks without a
/// module, the whole project, and a target that is gone.
fn target_name(db: &ProjectDb, target: WatchTarget) -> Result<Option<String>, StoreError> {
  Ok(match target {
    WatchTarget::Task(task) => recorded::task_of(db, task)?.map(|task| task.key().to_string()),
    WatchTarget::Module(module) => db
      .module(module)?
      .filter(|module| module.details().state() == ModuleState::Open)
      .map(|module| module.name().as_str().to_owned()),
    WatchTarget::WithoutModule | WatchTarget::Project => None,
  })
}

/// Raises, in the conversation of each subscription that leads to the end, the event its standing raises now, when
/// its state changed since the last look: work can start, or the target is stuck. `drives`: the person's switch,
/// which decides whether the event starts a turn by itself. Whether it kept one. Reads nothing while no subscription
/// leads to the end. Blocks on SQLite.
pub(super) fn raise(db: &mut ProjectDb, drives: bool, now: Timestamp) -> Result<bool, StoreError> {
  let leading: Vec<Subscription> = db
    .active_subscriptions()?
    .as_slice()
    .iter()
    .filter(|subscription| subscription.mode().ceilings().is_some())
    .cloned()
    .collect();
  if leading.is_empty() {
    return Ok(false);
  }
  let reads = Reads::read(db)?;
  let mut kept = false;
  for subscription in &leading {
    let led = reads.led(db, subscription)?;
    let raised = Raised::of(&led.standing, |task| {
      let parking = led.parked.iter().find(|(key, _)| key == task);
      parking.map(|(_, parking)| parking.to_do.as_str().to_owned())
    });
    let state = raised.as_ref().map_or_else(|| QUIET.to_owned(), Raised::state);
    if db.standing_state(subscription)?.as_deref() == Some(state.as_str()) {
      continue;
    }
    let event = raised.map(|raised| StandingEvent {
      target: subscription.target(),
      name: led.name.clone(),
      raised,
      automatic: drives,
    });
    db.keep_standing(subscription, (&state, event.as_ref()), now)?;
    kept |= event.is_some();
  }
  Ok(kept)
}

/// Every active subscription of the project at `root` with where its target stands. Blocks on SQLite.
pub fn standing_in(root: &Path) -> Result<Vec<Led>, Message> {
  let read = || -> Result<Vec<Led>, StoreError> {
    let db = ProjectDb::open_read_only(root)?;
    Reads::read(&db)?.all(&db)
  };
  read().map_err(|error| messages::store_error(&error))
}

/// What the line of an event of a module led to the end names of the module as it is now
/// (`docs/plan/orchestrator-autonomy/frames.md` H, d and e): its acceptance pipeline, and the branch that is
/// delivered with the branch it goes into.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleNow {
  /// The name of its acceptance pipeline.
  pub acceptance: Option<String>,
  /// Its own branch.
  pub branch: Option<String>,
  /// The branch it is delivered into, when the module names one; the checkout's branch otherwise.
  pub base: Option<String>,
}

/// The modules the events among `entries` are about, as they are now, by entry: for the events whose line names
/// something of the module (its acceptance can start, it can be delivered). An event of a module that is gone, or
/// that has no branch of its own any more, has none.
pub(super) fn modules_now(db: &ProjectDb, entries: &[ChatEntry]) -> Result<HashMap<i64, ModuleNow>, StoreError> {
  let mut now = HashMap::new();
  for entry in entries {
    let EntryKind::Standing { event, .. } = &entry.kind else {
      continue;
    };
    let (WatchTarget::Module(id), Raised::Accept | Raised::Deliver) = (event.target, &event.raised) else {
      continue;
    };
    let Some(module) = db.module(id)? else {
      continue;
    };
    let Some(own) = module.own_branch() else {
      continue;
    };
    let acceptance = match own.acceptance {
      Some(pipeline) => db
        .resource(pipeline)?
        .map(|pipeline| pipeline.name().as_str().to_owned()),
      None => None,
    };
    now.insert(
      entry.seq,
      ModuleNow {
        acceptance,
        branch: Some(own.branch.as_str().to_owned()),
        base: module.base().branch().map(|base| base.as_str().to_owned()),
      },
    );
  }
  Ok(now)
}

/// The words of `event`'s line in its conversation.
pub fn event_line(event: &StandingEvent) -> Message {
  let target = event.name.clone().unwrap_or_default();
  match &event.raised {
    Raised::CanStart { tasks } => Message::new("orchestrator.standing.canStart")
      .with("target", target)
      .with("tasks", tasks.join(", ")),
    Raised::Stuck { left, .. } => Message::new("orchestrator.standing.stuck")
      .with("target", target)
      .with("count", left.len().to_string()),
    Raised::Accept => Message::new("orchestrator.standing.accept").with("target", target),
    Raised::Deliver => Message::new("orchestrator.standing.deliver").with("target", target),
    Raised::DeliveryWaits { .. } => Message::new("orchestrator.standing.deliveryWaits").with("target", target),
  }
}
