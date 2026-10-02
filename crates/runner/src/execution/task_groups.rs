//! Advances reviewed groups and persists why unfinished tasks wait.
use std::{
  collections::HashMap,
  time::{Duration, Instant},
};

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  run::{Run, RunStatus},
  store::RunStore,
  task_group::{GroupHold, TaskGroup},
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

/// The first run of each task of `group`, in the group's order, each read once.
fn first_runs(db: &ProjectDb, group: &TaskGroup) -> Result<Vec<Option<Run>>, Rejection> {
  group
    .tasks()
    .iter()
    .map(|task| db.run_with_key(task.first_run_key()))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| read_failed("cannot read group runs", error))
}

fn statuses(runs: &[Option<Run>]) -> Vec<Option<RunStatus>> {
  runs
    .iter()
    .map(|run| run.as_ref().map(|run| run.status().clone()))
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
fn blocker_runs(db: &ProjectDb, by: &[TaskId]) -> Result<bool, Rejection> {
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
    if Self::stored_hold(db, &group)? == Some(GroupHold::SessionEnded) {
      return Err(Rejection::TaskGroupWaiting {
        hold: GroupHold::SessionEnded,
      });
    }
    let runs = first_runs(db, &group)?;
    // The task's own Pending first run is the one being started, so it counts as unstarted.
    let statuses: Vec<_> = group
      .tasks()
      .iter()
      .zip(statuses(&runs))
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
    for group in db
      .unfinished_task_groups()
      .map_err(|error| super::failed("cannot read task groups", &error))?
    {
      let stored = Self::stored_hold(&db, &group)?;
      if stored.as_ref().is_some_and(|hold| !hold.clears_on_its_own()) {
        continue;
      }
      let runs = first_runs(&db, &group)?;
      let statuses = statuses(&runs);
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
    let db = self.open()?;
    for group in db
      .unfinished_task_groups()
      .map_err(|error| super::failed("cannot read task groups", &error))?
    {
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
    let runs = first_runs(db, group)?;
    let statuses = statuses(&runs);
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
    // A failed worktree creation may have left a Pending run. Retry its own saved key before
    // looking for more drafts; Pending is already counted against the group limit. A blocked task
    // waits for its blocking tasks without holding the others.
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

  /// Holds the unstarted tasks of every group until a person starts one again. A group held by a
  /// failed task or its spent cap keeps that hold, so its tasks can still be taken back to the
  /// drafts.
  pub(super) fn hold_groups_after_session_end(&self, db: &ProjectDb) -> Result<(), Rejection> {
    for group in db
      .unfinished_task_groups()
      .map_err(|e| super::failed("cannot read task groups", &e))?
    {
      let runs = first_runs(db, &group)?;
      let unstarted = runs
        .iter()
        .any(|run| run.as_ref().is_none_or(|run| run.status() == &RunStatus::Pending));
      if !unstarted {
        continue;
      }
      let statuses = statuses(&runs);
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
    Rejection::NoAgentSlot { .. } => GroupHold::AgentCapacity,
    Rejection::DailyBudgetUsed { .. } => GroupHold::DailyBudget,
    Rejection::Preflight { problems } => GroupHold::Preflight(problems.clone()),
    Rejection::LimitNotRaised {
      limit: orchester_core::run::RunLimit::GroupBudget,
    } => GroupHold::GroupCap,
    _ => GroupHold::StartFailed,
  }
}
