//! Advances reviewed groups and persists why unfinished tasks wait.
use std::{
  collections::{HashMap, HashSet},
  path::Path,
  time::{Duration, Instant},
};

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  run::{Run, RunStatus},
  store::{RunStore, WorktreeStore},
  task_group::{GroupHold, QueueHolds, TaskGroup},
  tokens::TokensUsed,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use parking_lot::Mutex;

use super::{Scheduler, read_failed};

/// A group whose start failed tries again at the next pass; when it fails again it waits
/// [`RETRY_FIRST`], doubling with each failure in a row up to [`RETRY_MAX`], so a start the person
/// must fix is not retried every second while a passing failure costs no wait.
const RETRY_FIRST: Duration = Duration::from_secs(2);
const RETRY_MAX: Duration = Duration::from_secs(60);

/// When each group whose next start failed may try again, and the last error each group's pass
/// ended with, so a lasting error is logged once rather than every pass.
#[derive(Default)]
pub(super) struct GroupRetries {
  retries: Mutex<HashMap<IdempotencyKey, Retry>>,
  errors: Mutex<HashMap<IdempotencyKey, Rejection>>,
}

#[derive(Clone, Copy)]
struct Retry {
  wait: Duration,
  due: Instant,
}

impl GroupRetries {
  fn due(&self, group: IdempotencyKey) -> bool {
    self
      .retries
      .lock()
      .get(&group)
      .is_none_or(|retry| Instant::now() >= retry.due)
  }

  fn failed(&self, group: IdempotencyKey) {
    let mut retries = self.retries.lock();
    let wait = retries
      .get(&group)
      .map_or(Duration::ZERO, |retry| (retry.wait * 2).clamp(RETRY_FIRST, RETRY_MAX));
    retries.insert(
      group,
      Retry {
        wait,
        due: Instant::now() + wait,
      },
    );
  }

  fn clear(&self, group: IdempotencyKey) {
    self.retries.lock().remove(&group);
  }

  /// Records how the group's pass ended; returns its error when it differs from the last pass's.
  fn passed(&self, group: IdempotencyKey, outcome: Result<(), Rejection>) -> Option<Rejection> {
    let mut errors = self.errors.lock();
    match outcome {
      Ok(()) => {
        errors.remove(&group);
        None
      }
      Err(error) if errors.get(&group) == Some(&error) => None,
      Err(error) => {
        errors.insert(group, error.clone());
        Some(error)
      }
    }
  }
}

/// The groups queued before this runner started. Only they can be left from an earlier session, so
/// only they are held when the desktop attaches for a new one: a group the desktop queued while this
/// runner was starting belongs to the new session and goes on.
pub(super) struct EarlierGroups(Option<HashSet<IdempotencyKey>>);

impl EarlierGroups {
  /// Read as the runner starts; when they cannot be read, every unfinished group counts as earlier.
  pub(super) fn read(project_root: &Path) -> Self {
    let keys = ProjectDb::open(project_root)
      .and_then(|db| db.unfinished_task_groups())
      .map(|groups| groups.iter().map(TaskGroup::key).collect());
    match keys {
      Ok(keys) => Self(Some(keys)),
      Err(error) => {
        tracing::warn!(%error, "cannot read the groups queued before the runner started; all count as earlier");
        Self(None)
      }
    }
  }

  fn contain(&self, group: IdempotencyKey) -> bool {
    self.0.as_ref().is_none_or(|keys| keys.contains(&group))
  }
}

/// The first run of each task of `group`, in the group's order, each read once.
fn first_runs(db: &ProjectDb, group: &TaskGroup) -> Result<Vec<Option<Run>>, Rejection> {
  group
    .tasks()
    .iter()
    .map(|task| db.run_with_key(task.first_run_key()))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| read_failed("cannot read group runs", error))
}

/// A group as its scheduling sees it, with the first run of each of its tasks.
type Scheduled = (TaskGroup, Vec<Option<Run>>);

/// The group as its scheduling sees it: without its closed (Done or Canceled) tasks that never started
/// ([`TaskGroup::skipping`]), with the first runs of the others in order; `None` when every task
/// is skipped, which finishes the group.
fn scheduled(db: &ProjectDb, group: &TaskGroup) -> Result<Option<Scheduled>, Rejection> {
  let runs = first_runs(db, group)?;
  let mut skipped = Vec::new();
  for (task, run) in group.tasks().iter().zip(&runs) {
    let unstarted = run.as_ref().is_none_or(|run| run.status() == &RunStatus::Pending);
    if unstarted
      && db
        .task_organization(*task)
        .map_err(|error| super::failed("cannot read a task's status", &error))?
        .status()
        .is_closed()
    {
      skipped.push(*task);
    }
  }
  let Some(view) = group.skipping(&skipped) else {
    return Ok(None);
  };
  let runs = group
    .tasks()
    .iter()
    .zip(runs)
    .filter(|(task, _)| !skipped.contains(task))
    .map(|(_, run)| run)
    .collect();
  Ok(Some((view, runs)))
}

/// Where the group's first runs stand. An interrupted run whose worktree was removed never
/// continues (ORC-63): for the group it ended as a cancelled run does, so it holds no slot.
fn statuses(db: &ProjectDb, runs: &[Option<Run>]) -> Result<Vec<Option<RunStatus>>, Rejection> {
  runs
    .iter()
    .map(|run| {
      let Some(run) = run else {
        return Ok(None);
      };
      let ended = matches!(run.status(), RunStatus::Interrupted(_))
        && db
          .worktree_removal(run.id())
          .map_err(|e| super::failed("cannot read whether a run's worktree was removed", &e))?
          .is_some();
      Ok(Some(if ended {
        RunStatus::Cancelled
      } else {
        run.status().clone()
      }))
    })
    .collect()
}

/// What the group's first runs used together, as `ProjectDb::task_group_usage` counts it.
fn usage(runs: &[Option<Run>]) -> TokensUsed {
  runs.iter().flatten().fold(TokensUsed::ZERO, |mut used, run| {
    used += run.used();
    used
  })
}

/// The unfinished blocking tasks of each task of `group` that has not started, in the group's
/// order; empty for a task that is free to start or already runs.
fn blockers(db: &ProjectDb, group: &TaskGroup, statuses: &[Option<RunStatus>]) -> Result<Vec<Vec<TaskId>>, Rejection> {
  group
    .tasks()
    .iter()
    .zip(statuses)
    .map(|(task, status)| match status {
      None | Some(RunStatus::Pending) => db
        .unfinished_blocking_tasks(*task)
        .map_err(|error| super::failed("cannot read blocking tasks", &error)),
      Some(_) => Ok(Vec::new()),
    })
    .collect()
}

/// Whether a blocking task runs now, so it finishes without a person and the group behind it
/// will go on.
pub(super) fn blocker_runs(db: &ProjectDb, by: &[TaskId]) -> Result<bool, Rejection> {
  for task in by {
    let active = db
      .active_task_run(*task)
      .map_err(|error| super::failed("cannot read a blocking task's run", &error))?;
    let Some(run) = active else {
      continue;
    };
    let run = db
      .run(run)
      .map_err(|error| read_failed("cannot read a blocking task's run", error))?;
    if run.is_some_and(|run| matches!(run.status(), RunStatus::Pending | RunStatus::Running)) {
      return Ok(true);
    }
  }
  Ok(false)
}

/// A start that failed this way waits for the person; the group retries it with a backoff.
const fn retries_later(hold: &GroupHold) -> bool {
  matches!(hold, GroupHold::Preflight(_) | GroupHold::StartFailed)
}

impl Scheduler {
  fn queue_holds(db: &ProjectDb) -> Result<QueueHolds, Rejection> {
    db.queue_holds()
      .map_err(|e| super::failed("cannot read the holds on new starts", &e))
  }

  fn stored_hold(db: &ProjectDb, group: &TaskGroup) -> Result<Option<GroupHold>, Rejection> {
    db.task_group_hold(group.key())
      .map_err(|e| super::failed("cannot read queue hold", &e))
  }

  /// Single-task start requests cannot bypass the saved group review.
  pub(super) fn check_group_start(
    &self,
    db: &ProjectDb,
    task: TaskId,
    recorded: Option<&Run>,
  ) -> Result<(), Rejection> {
    // A person's hold on new starts holds every first run in the project, in a group or not.
    let holds = Self::queue_holds(db)?;
    if let Some(hold) = holds.of(None) {
      return Err(Rejection::TaskGroupWaiting { hold });
    }
    if let Some(run) = recorded
      && db
        .task_group_for_run(run)
        .map_err(|e| read_failed("cannot read run group", e))?
        .is_none()
    {
      return Ok(());
    }
    let Some(group) = db
      .task_group_for_task(task)
      .map_err(|e| super::failed("cannot read task group", &e))?
    else {
      return Ok(());
    };
    if let Some(hold) = holds.of(Some(group.key())) {
      return Err(Rejection::TaskGroupWaiting { hold });
    }
    if Self::stored_hold(db, &group)? == Some(GroupHold::SessionEnded) {
      return Err(Rejection::TaskGroupWaiting {
        hold: GroupHold::SessionEnded,
      });
    }
    // A closed (Done or Canceled) task is not started by its group.
    let canceled = Rejection::TaskGroupWaiting {
      hold: GroupHold::Canceled,
    };
    let Some((group, runs)) = scheduled(db, &group)? else {
      return Err(canceled);
    };
    if !group.tasks().contains(&task) {
      return Err(canceled);
    }
    // The task's own Pending first run is the one being started, so it counts as unstarted.
    let statuses: Vec<_> = group
      .tasks()
      .iter()
      .zip(statuses(db, &runs)?)
      .map(|(id, status)| status.filter(|status| !(*id == task && status == &RunStatus::Pending)))
      .collect();
    let used = usage(&runs);
    let blockers = blockers(db, &group, &statuses)?;
    if group.eligible(&statuses, used, &blockers).contains(&task) {
      return Ok(());
    }
    Err(Rejection::TaskGroupWaiting {
      hold: group.waiting_reason(&statuses, used, &blockers),
    })
  }

  /// Whether a group has unstarted tasks that start, or will start once a hold ends on its own.
  /// A group held for a person (a failed task, the spent cap, a start to fix, a reopened project)
  /// does not keep the runner busy.
  pub(super) fn has_pending_task_groups(&self) -> Result<bool, Rejection> {
    let db = self.open()?;
    let holds = Self::queue_holds(&db)?;
    for group in db
      .unfinished_task_groups()
      .map_err(|error| super::failed("cannot read task groups", &error))?
    {
      // A person's hold ends only when a person lets the group go on.
      if holds.of(Some(group.key())).is_some() {
        continue;
      }
      let stored = Self::stored_hold(&db, &group)?;
      if stored.as_ref().is_some_and(|hold| !hold.clears_on_its_own()) {
        continue;
      }
      let Some((group, runs)) = scheduled(&db, &group)? else {
        continue;
      };
      let statuses = statuses(&db, &runs)?;
      let (used, blockers) = (usage(&runs), blockers(&db, &group, &statuses)?);
      // The free slot or the day's budget the runner waits for comes without a person.
      let waits_for_capacity = matches!(stored, Some(GroupHold::AgentCapacity | GroupHold::DailyBudget))
        && statuses.iter().any(Option::is_none);
      if waits_for_capacity || group.has_work_of_its_own(&statuses, used, &blockers) {
        return Ok(true);
      }
      // Tasks waiting for their blocking tasks go on when a blocking task that runs now finishes.
      if let Some(GroupHold::Blocked { by }) = group.hold(&statuses, used, &blockers)
        && blocker_runs(&db, &by)?
      {
        return Ok(true);
      }
    }
    Ok(false)
  }

  pub async fn advance_task_groups(&self) -> Result<(), Rejection> {
    if self.inner.board.slots().is_none() || self.inner.board.is_stopping() {
      return Ok(());
    }
    // Runs whose messages waited for blocking tasks go on once those finish.
    if let Err(error) = self.release_unblocked().await {
      tracing::error!(?error, "cannot deliver messages that waited for blocking tasks");
    }
    let db = self.open()?;
    let holds = Self::queue_holds(&db)?;
    for group in db
      .unfinished_task_groups()
      .map_err(|error| super::failed("cannot read task groups", &error))?
    {
      // A held group starts nothing; its tasks show why until a person lets it go on.
      if let Some(hold) = holds.of(Some(group.key())) {
        self.group_hold(&db, &group, Some(&hold))?;
        continue;
      }
      // One group's failure leaves the others to go on; the next pass tries it again, and the
      // same failure again is not logged again.
      let outcome = self.advance_group(&db, &group).await;
      if let Some(error) = self.inner.group_retries.passed(group.key(), outcome) {
        tracing::error!(?error, group = %group.key(), "cannot advance a reviewed task group");
      }
    }
    Ok(())
  }

  async fn advance_group(&self, db: &ProjectDb, group: &TaskGroup) -> Result<(), Rejection> {
    let stored = Self::stored_hold(db, group)?;
    if stored == Some(GroupHold::SessionEnded) {
      return Ok(());
    }
    let Some((view, runs)) = scheduled(db, group)? else {
      self.inner.group_retries.clear(group.key());
      return db
        .finish_task_group(group.key())
        .map_err(|e| super::failed("cannot record a finished group", &e));
    };
    let group = &view;
    let statuses = statuses(db, &runs)?;
    if TaskGroup::finished(&statuses) {
      self.inner.group_retries.clear(group.key());
      return db
        .finish_task_group(group.key())
        .map_err(|e| super::failed("cannot record a finished group", &e));
    }
    if stored.as_ref().is_some_and(retries_later) && !self.inner.group_retries.due(group.key()) {
      return Ok(());
    }
    let used = usage(&runs);
    let blockers = blockers(db, group, &statuses)?;
    // A start that stopped between recording the run and starting it (the runner ended, a database
    // write failed) left a Pending run. Retry its own saved key before looking for more drafts;
    // Pending is already counted against the group limit. A run whose worktree could not be created
    // failed instead, which holds the group for the person. A blocked task waits for its blocking
    // tasks without holding the others.
    let pending = group
      .tasks()
      .iter()
      .zip(&statuses)
      .zip(&blockers)
      .filter(|((_, s), by)| matches!(s, Some(RunStatus::Pending)) && by.is_empty())
      .map(|((id, _), _)| *id);
    for task in pending.chain(group.eligible(&statuses, used, &blockers)) {
      match self.start_run(task.first_run_key(), task).await {
        Ok(_) => {}
        // Blocked since the blocking tasks were read: it waits like the other blocked tasks.
        Err(Rejection::RunAgainBlocked {
          block: orchester_core::run::RunAgainBlock::TaskBlocked,
        }) => {}
        Err(error) => return self.record_failed_start(db, group, &error),
      }
    }
    self.inner.group_retries.clear(group.key());
    self.group_hold(db, group, group.hold(&statuses, used, &blockers).as_ref())?;
    Ok(())
  }

  /// Records why the group's next task did not start, logging it only when the reason changes,
  /// and backs off a start the person must fix.
  fn record_failed_start(&self, db: &ProjectDb, group: &TaskGroup, error: &Rejection) -> Result<(), Rejection> {
    let hold = rejection_hold(error);
    if retries_later(&hold) {
      self.inner.group_retries.failed(group.key());
    }
    if self.group_hold(db, group, Some(&hold))? {
      tracing::warn!(?error, group = %group.key(), "reviewed group could not start its next task");
    }
    Ok(())
  }

  /// A person's Start request releases the session hold; automatic scheduling never does.
  pub async fn start_requested_run(
    &self,
    key: IdempotencyKey,
    task: TaskId,
  ) -> Result<orchester_core::id::RunId, Rejection> {
    let db = self.open()?;
    if key == task.first_run_key() {
      let first = db
        .run_with_key(key)
        .map_err(|e| read_failed("cannot read first run", e))?;
      if first.is_none_or(|run| run.status() == &RunStatus::Pending) {
        self.release_group_session_hold(&db, task)?;
      }
    }
    self.start_run(key, task).await
  }

  pub(super) fn release_group_session_hold(&self, db: &ProjectDb, task: TaskId) -> Result<(), Rejection> {
    if let Some(group) = db
      .task_group_for_task(task)
      .map_err(|e| super::failed("cannot read task group", &e))?
      && Self::stored_hold(db, &group)? == Some(GroupHold::SessionEnded)
    {
      self.group_hold(db, &group, None)?;
    }
    Ok(())
  }

  /// Holds the unstarted tasks of every group queued before this runner started ([`EarlierGroups`])
  /// until a person starts one again. A group held by a failed task or its spent cap keeps that hold,
  /// so its tasks can still be taken back to the drafts.
  pub(super) fn hold_groups_after_session_end(&self, db: &ProjectDb) -> Result<(), Rejection> {
    for group in db
      .unfinished_task_groups()
      .map_err(|e| super::failed("cannot read task groups", &e))?
    {
      if !self.inner.earlier_groups.contain(group.key()) {
        continue;
      }
      let Some((group, runs)) = scheduled(db, &group)? else {
        continue;
      };
      let unstarted = runs
        .iter()
        .any(|run| run.as_ref().is_none_or(|run| run.status() == &RunStatus::Pending));
      if !unstarted {
        continue;
      }
      let statuses = statuses(db, &runs)?;
      let blockers = blockers(db, &group, &statuses)?;
      let lasting = group
        .hold(&statuses, usage(&runs), &blockers)
        .filter(GroupHold::outlives_session)
        .or(Self::stored_hold(db, &group)?.filter(GroupHold::outlives_session));
      let hold = lasting.unwrap_or(GroupHold::SessionEnded);
      self.group_hold(db, &group, Some(&hold))?;
    }
    Ok(())
  }

  /// Stores the group's hold; returns whether it changed, in which case the desktop is told.
  fn group_hold(&self, db: &ProjectDb, group: &TaskGroup, hold: Option<&GroupHold>) -> Result<bool, Rejection> {
    let changed = db
      .set_task_group_hold(group.key(), hold)
      .map_err(|error| super::failed("cannot record queue hold", &error))?;
    if changed {
      self.inner.notifier.send(Event::TaskGroupChanged { key: group.key() });
    }
    Ok(changed)
  }
}

fn rejection_hold(error: &Rejection) -> GroupHold {
  match error {
    Rejection::TaskGroupWaiting { hold } => hold.clone(),
    // The run failed before it started; retrying its key answers the same, so the person decides.
    Rejection::RunNotStarted { .. } => GroupHold::FailedTask,
    Rejection::NoAgentSlot { .. } | Rejection::LowDisk { .. } => GroupHold::AgentCapacity,
    Rejection::DailyBudgetUsed { .. } => GroupHold::DailyBudget,
    Rejection::Preflight { problems } => GroupHold::Preflight(problems.clone()),
    Rejection::LimitNotRaised {
      limit: orchester_core::run::RunLimit::GroupBudget,
    } => GroupHold::GroupCap,
    _ => GroupHold::StartFailed,
  }
}
