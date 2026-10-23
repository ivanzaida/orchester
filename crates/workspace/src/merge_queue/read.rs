//! The queue as the Merge queue screen, the sidebar, the task page and automation read it: its runs in order with
//! their tasks, pipelines and what happened to them, the base branch and how the person's checkout stands, and, for an
//! empty queue, how the project's pipelines deliver. A run that left the queue or was merged shows on the day it did.

use std::path::Path;

use orchester_core::{
  id::{ResourceId, RunId, TaskId},
  integration::{
    branch_head,
    delivery::{self, Checkout},
  },
  merge_queue::{QueueEntry, QueueEvent, QueueState, RecordedQueueEvent},
  resource::{DeliverMode, DeliverStep, Pipeline, ResourceKind},
  revision::Revision,
  run::BranchName,
  store::{ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::{changes::GIT, detail::run_store_error};

/// The queue of a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueRead {
  /// Where its runs deliver, when one names a base branch or the checkout has one checked out.
  pub base: Option<BaseState>,
  /// Its runs in the queue's order: the one being delivered, the waiting ones, those waiting for approval, and the
  /// ones that left it or were merged today.
  pub rows: Vec<QueueRow>,
  /// How the project's published pipelines deliver, for an empty queue.
  pub pipelines: Vec<PipelineDelivery>,
}

impl QueueRead {
  /// The runs in the queue: not merged, not out of it.
  pub fn in_queue(&self) -> usize {
    self.rows.iter().filter(|row| row.entry.state.in_queue()).count()
  }
}

/// The base branch the queue delivers into and how the person's checkout stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseState {
  pub branch: BranchName,
  pub head: Option<Revision>,
  pub checkout: CheckoutState,
}

/// How the person's checkout stands for delivering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckoutState {
  Clean,
  /// These files have uncommitted changes (`git status` lines).
  Uncommitted(Vec<String>),
  OtherBranch(Option<String>),
  InProgress,
  /// Git could not tell.
  Unknown,
}

/// One run of the queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueRow {
  pub entry: QueueEntry,
  pub task: TaskId,
  pub task_key: orchester_core::project::TaskKey,
  pub task_title: String,
  pub run_number: u32,
  pub branch: BranchName,
  pub base_branch: Option<BranchName>,
  /// Its pipeline's name and the version the run pinned.
  pub pipeline: (String, u32),
  pub deliver: DeliverStep,
  /// The integrator profile's name, when one is chosen.
  pub integrator: Option<String>,
  /// What happened to it in the queue, oldest first.
  pub events: Vec<RecordedQueueEvent>,
  /// Its place among the runs waiting their turn, from 1.
  pub place: Option<usize>,
  /// Waiting for approval, its base moved since its check: merging syncs and checks again first.
  pub base_moved: bool,
}

/// A step of a run's delivery as its row and its task page show it: *Synced with main › Check on the merged result ›
/// Your approval › Merge into main*, with *Integrate* after a sync that conflicted and the superseded check before a
/// re-sync.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowPhase {
  /// Syncing with the base (at `base` once known): `conflicts` files conflicted when it is done.
  Sync {
    state: PhaseState,
    base: Option<Revision>,
    conflicts: usize,
  },
  /// The check on the base at `base` no longer matches what would land: the base moved to `now`.
  Superseded { base: Revision, now: Revision },
  Integrate { state: PhaseState },
  /// Check on the merged result; running since `since`.
  Check { state: PhaseState, since: Option<Timestamp> },
  /// A person's approval, given at `at`.
  Approval { state: PhaseState, at: Option<Timestamp> },
  Merge { state: PhaseState },
}

/// Where a phase stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhaseState {
  Done,
  Active,
  Pending,
  Blocked,
}

impl QueueRow {
  /// The phases of the run's delivery while it is being delivered or waits for approval; none otherwise.
  pub fn phases(&self) -> Vec<RowPhase> {
    phases(&self.entry, &self.events, &self.deliver)
  }
}

impl RunDelivery {
  /// The phases of the run's delivery as its pipeline's Deliver step `deliver` has them ([`QueueRow::phases`]).
  pub fn phases(&self, deliver: &DeliverStep) -> Vec<RowPhase> {
    phases(&self.entry, &self.events, deliver)
  }
}

/// The phases of `entry`'s delivery, with what happened to it (`events`), as `deliver` delivers.
fn phases(entry: &QueueEntry, events: &[RecordedQueueEvent], deliver: &DeliverStep) -> Vec<RowPhase> {
  let synced = last_sync(events);
  let done_sync = || RowPhase::Sync {
    state: PhaseState::Done,
    base: synced.as_ref().map(|(base, _)| base.clone()),
    conflicts: synced.as_ref().map_or(0, |(_, conflicts)| *conflicts),
  };
  let approval = |state| RowPhase::Approval {
    state,
    at: entry.approved_at,
  };
  let mut phases = match &entry.state {
    QueueState::Syncing => syncing(entry, events),
    QueueState::Rechecking { .. } => vec![done_sync(), check(PhaseState::Active, Some(entry.since))],
    QueueState::Integrating { .. } => vec![
      done_sync(),
      RowPhase::Integrate {
        state: PhaseState::Active,
      },
      check(PhaseState::Pending, None),
    ],
    QueueState::WaitingForApproval | QueueState::Blocked(_) => vec![done_sync(), check(PhaseState::Done, None)],
    _ => return Vec::new(),
  };
  if deliver.mode() == DeliverMode::ApproveToMerge {
    phases.push(match &entry.state {
      QueueState::WaitingForApproval => approval(PhaseState::Active),
      QueueState::Blocked(_) if entry.approved_at.is_some() => approval(PhaseState::Done),
      _ => approval(PhaseState::Pending),
    });
  }
  let merge = if matches!(entry.state, QueueState::Blocked(_)) {
    PhaseState::Blocked
  } else {
    PhaseState::Pending
  };
  phases.push(RowPhase::Merge { state: merge });
  phases
}

/// Syncing: after a check the base superseded, the sync before it and that check first.
fn syncing(entry: &QueueEntry, events: &[RecordedQueueEvent]) -> Vec<RowPhase> {
  let superseded = (entry.resyncs > 0)
    .then(|| {
      events.iter().rev().find_map(|recorded| match &recorded.event {
        QueueEvent::CheckSuperseded { base, now } => Some((base.clone(), now.clone())),
        _ => None,
      })
    })
    .flatten();
  let mut phases = Vec::new();
  if let Some((base, now)) = superseded {
    phases.push(RowPhase::Sync {
      state: PhaseState::Done,
      base: Some(base.clone()),
      conflicts: 0,
    });
    phases.push(RowPhase::Superseded { base, now });
  }
  phases.push(RowPhase::Sync {
    state: PhaseState::Active,
    base: None,
    conflicts: 0,
  });
  phases.push(check(PhaseState::Pending, None));
  phases
}

/// The base the run was last synced with and how many files conflicted then.
fn last_sync(events: &[RecordedQueueEvent]) -> Option<(Revision, usize)> {
  events.iter().rev().find_map(|recorded| match &recorded.event {
    QueueEvent::Synced { base, files } => Some((base.clone(), files.len())),
    _ => None,
  })
}

const fn check(state: PhaseState, since: Option<Timestamp>) -> RowPhase {
  RowPhase::Check { state, since }
}

/// How a published pipeline delivers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineDelivery {
  pub id: ResourceId,
  pub name: String,
  pub version: u32,
  pub deliver: Option<DeliverStep>,
}

/// A run's place in its project's queue, as its task page and automation read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunDelivery {
  pub entry: QueueEntry,
  pub events: Vec<RecordedQueueEvent>,
  /// Its place among the runs in the queue, from 1, while it is in it.
  pub place: Option<usize>,
}

/// The queue of the project at `root`, with `programs`' git telling how the checkout stands. Blocks on SQLite and git.
pub fn read(root: &Path, programs: &Programs) -> Result<QueueRead, StoreError> {
  let db = ProjectDb::open(root)?;
  let today = Timestamp::now().local_day_start();
  let entries: Vec<QueueEntry> = db
    .merge_queue()?
    .into_iter()
    .filter(|entry| entry.state.in_queue() || (entry.state != QueueState::Removed && entry.since >= today))
    .collect();
  let waiting: Vec<RunId> = entries
    .iter()
    .filter(|entry| entry.state == QueueState::Queued)
    .map(|entry| entry.run)
    .collect();
  let mut rows = Vec::with_capacity(entries.len());
  for entry in entries {
    let place = waiting.iter().position(|run| *run == entry.run).map(|index| index + 1);
    if let Some(row) = row(&db, entry, place)? {
      rows.push(row);
    }
  }
  let base = base_branch(&rows, (root, programs));
  let base = base.map(|branch| base_state(branch, (root, programs)));
  let rows = rows
    .into_iter()
    .map(|mut row| {
      row.base_moved = moved_since_check(&row, base.as_ref());
      row
    })
    .collect();
  Ok(QueueRead {
    base,
    rows,
    pipelines: pipelines(&db)?,
  })
}

/// Whether the queue delivers `run`, so that neither the task page's *Merge into <base>* nor `merge_run` merges it:
/// its pipeline, as it pinned it, ends in a Deliver step, unless a person took it out of the queue (then it is merged
/// from its task page as any run). The Orchestrator's switch to merge finished runs on its own applies only to the
/// others.
pub fn delivered_by_queue(db: &ProjectDb, run: &orchester_core::run::Run) -> Result<bool, StoreError> {
  if run.snapshot().body().deliver().is_none() {
    return Ok(false);
  }
  Ok(
    db.merge_queue_entry(run.id())?
      .is_none_or(|entry| entry.state != QueueState::Removed),
  )
}

/// How the latest run of a task is delivered, as its task page shows it: its pipeline's Deliver step as the run
/// pinned it, the integrator profile's name, and its place in the queue once it joined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDelivery {
  pub deliver: DeliverStep,
  pub integrator: Option<String>,
  pub delivery: Option<RunDelivery>,
}

/// How `run` is delivered, when its pipeline ends in a Deliver step.
pub fn task_delivery(db: &ProjectDb, run: &orchester_core::run::Run) -> Result<Option<TaskDelivery>, StoreError> {
  let Some(deliver) = run.snapshot().body().deliver().cloned() else {
    return Ok(None);
  };
  Ok(Some(TaskDelivery {
    integrator: integrator_name(db, &deliver)?,
    delivery: delivery_of(db, run.id())?,
    deliver,
  }))
}

fn integrator_name(db: &ProjectDb, deliver: &DeliverStep) -> Result<Option<String>, StoreError> {
  Ok(
    deliver
      .integrator()
      .map(|profile| db.resource(profile))
      .transpose()?
      .flatten()
      .map(|resource| resource.name().as_str().to_owned()),
  )
}

/// `run`'s place in the queue of the project in `db`, when it ever joined it.
pub fn delivery_of(db: &ProjectDb, run: RunId) -> Result<Option<RunDelivery>, StoreError> {
  let Some(entry) = db.merge_queue_entry(run)? else {
    return Ok(None);
  };
  let place = entry
    .state
    .in_queue()
    .then(|| db.merge_queue())
    .transpose()?
    .and_then(|entries| {
      let mut queue: Vec<&QueueEntry> = entries.iter().filter(|other| other.state.in_queue()).collect();
      queue.sort_by_key(|other| !other.state.delivering());
      queue.iter().position(|other| other.run == run).map(|index| index + 1)
    });
  Ok(Some(RunDelivery {
    events: db.merge_queue_events(run)?,
    entry,
    place,
  }))
}

fn row(db: &ProjectDb, entry: QueueEntry, place: Option<usize>) -> Result<Option<QueueRow>, StoreError> {
  let Some(run) = db.run(entry.run).map_err(run_store_error)? else {
    return Ok(None);
  };
  let Some(task) = db.task(entry.task)? else {
    return Ok(None);
  };
  let Some(deliver) = run.snapshot().body().deliver().cloned() else {
    return Ok(None);
  };
  let pinned = run.snapshot().pipeline();
  let pipeline = db
    .resource(pinned.resource)?
    .map_or_else(String::new, |resource| resource.name().as_str().to_owned());
  let integrator = integrator_name(db, &deliver)?;
  Ok(Some(QueueRow {
    task: task.id(),
    task_key: task.key().clone(),
    task_title: task.details().title.as_str().to_owned(),
    run_number: run.number().get(),
    branch: run.branch().clone(),
    base_branch: run.base_branch().cloned(),
    pipeline: (pipeline, pinned.number.get()),
    deliver,
    integrator,
    events: db.merge_queue_events(entry.run)?,
    place,
    base_moved: false,
    entry,
  }))
}

/// The base branch the queue delivers into: that of the first run in it that names one, else of any run shown, else
/// the branch the checkout has checked out.
fn base_branch(rows: &[QueueRow], (root, programs): (&Path, &Programs)) -> Option<BranchName> {
  let named = |in_queue: bool| {
    rows
      .iter()
      .filter(|row| row.entry.state.in_queue() == in_queue)
      .find_map(|row| row.base_branch.clone())
  };
  named(true).or_else(|| named(false)).or_else(|| {
    orchester_core::integration::checked_out_branch(&programs.runner(), Path::new(GIT), root)
      .ok()
      .flatten()
  })
}

fn base_state(branch: BranchName, (root, programs): (&Path, &Programs)) -> BaseState {
  let runner = programs.runner();
  let head = branch_head(&runner, Path::new(GIT), (root, &branch)).ok().flatten();
  let checkout = match delivery::checkout(&runner, Path::new(GIT), (root, &branch)) {
    Ok(Checkout::OnBase { uncommitted }) if uncommitted.is_empty() => CheckoutState::Clean,
    Ok(Checkout::OnBase { uncommitted }) => CheckoutState::Uncommitted(uncommitted),
    Ok(Checkout::OtherBranch(current)) => {
      CheckoutState::OtherBranch(current.map(|branch| branch.as_str().to_owned()))
    }
    Ok(Checkout::InProgress) => CheckoutState::InProgress,
    Err(error) => {
      tracing::warn!(%error, "cannot tell how the checkout stands for delivering");
      CheckoutState::Unknown
    }
  };
  BaseState { branch, head, checkout }
}

/// Whether `row`, waiting for approval, was checked on a base that moved since.
fn moved_since_check(row: &QueueRow, base: Option<&BaseState>) -> bool {
  let (Some(checked), Some(base)) = (row.entry.checked.as_ref(), base) else {
    return false;
  };
  row.entry.state == QueueState::WaitingForApproval
    && row.base_branch.as_ref() == Some(&base.branch)
    && base.head.as_ref().is_some_and(|head| *head != checked.base)
}

fn pipelines(db: &ProjectDb) -> Result<Vec<PipelineDelivery>, StoreError> {
  let mut listed = Vec::new();
  for resource in db.resources(ResourceKind::Pipeline)? {
    let Some(version) = db.latest_version::<Pipeline>(resource.id())? else {
      continue;
    };
    listed.push(PipelineDelivery {
      id: resource.id(),
      name: resource.name().as_str().to_owned(),
      version: version.number().get(),
      deliver: version.body().deliver().cloned(),
    });
  }
  Ok(listed)
}
