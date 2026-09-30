//! Advances reviewed groups and persists why unfinished tasks wait.
use orchester_core::{
  run::RunStatus,
  store::RunStore,
  task_group::{GroupHold, Scheduling, TaskGroup},
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;

use super::{Scheduler, read_failed};

impl Scheduler {
  /// Single-task start requests cannot bypass the saved group review.
  pub(super) fn check_group_start(&self, db: &ProjectDb, task: orchester_core::id::TaskId) -> Result<(), Rejection> {
    let Some(group) = db
      .task_group_for_task(task)
      .map_err(|e| super::failed("cannot read task group", &e))?
    else {
      return Ok(());
    };
    let statuses = group
      .tasks()
      .iter()
      .map(|id| {
        db.run_with_key(id.first_run_key()).map(|run| {
          run.and_then(|run| {
            if *id == task && run.status() == &RunStatus::Pending {
              None
            } else {
              Some(run.status().clone())
            }
          })
        })
      })
      .collect::<Result<Vec<_>, _>>()
      .map_err(|e| read_failed("cannot read group runs", e))?;
    let used = db
      .task_group_usage(&group)
      .map_err(|e| read_failed("cannot read group usage", e))?;
    if group.eligible(&statuses, used).contains(&task) {
      return Ok(());
    }
    let hold = if used.reached(group.cap()) {
      GroupHold::GroupCap
    } else if statuses
      .iter()
      .flatten()
      .any(|s| matches!(s, RunStatus::Failed(_) | RunStatus::Cancelled))
    {
      GroupHold::FailedTask
    } else {
      match group.scheduling() {
        Scheduling::Sequential => GroupHold::PreviousTask,
        Scheduling::Parallel(_) => GroupHold::Concurrency,
      }
    };
    Err(Rejection::TaskGroupWaiting { hold })
  }

  pub(super) fn has_pending_task_groups(&self) -> Result<bool, Rejection> {
    let db = self.open()?;
    for group in db
      .task_groups()
      .map_err(|error| super::failed("cannot read task groups", &error))?
    {
      for task in group.tasks() {
        if db
          .run_with_key(task.first_run_key())
          .map_err(|error| read_failed("cannot read group runs", error))?
          .is_none()
        {
          return Ok(true);
        }
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
      .task_groups()
      .map_err(|error| super::failed("cannot read task groups", &error))?
    {
      self.advance_group(&db, &group).await?;
    }
    Ok(())
  }

  async fn advance_group(&self, db: &ProjectDb, group: &TaskGroup) -> Result<(), Rejection> {
    let statuses = group
      .tasks()
      .iter()
      .map(|task| {
        db.run_with_key(task.first_run_key())
          .map(|run| run.map(|run| run.status().clone()))
      })
      .collect::<Result<Vec<_>, _>>()
      .map_err(|error| read_failed("cannot read group runs", error))?;
    let used = db
      .task_group_usage(group)
      .map_err(|error| read_failed("cannot read group usage", error))?;
    let eligible = group.eligible(&statuses, used);
    let hold = if used.reached(group.cap()) {
      Some(GroupHold::GroupCap)
    } else if statuses
      .iter()
      .flatten()
      .any(|s| matches!(s, RunStatus::Failed(_) | RunStatus::Cancelled))
    {
      Some(GroupHold::FailedTask)
    } else if statuses.iter().any(Option::is_none) && eligible.is_empty() {
      Some(match group.scheduling() {
        Scheduling::Sequential => GroupHold::PreviousTask,
        Scheduling::Parallel(_) => GroupHold::Concurrency,
      })
    } else {
      None
    };
    // A failed worktree creation may have left a Pending run. Retry its own saved key before
    // looking for more drafts; Pending is already counted against the group limit.
    let pending = group
      .tasks()
      .iter()
      .zip(&statuses)
      .filter(|(_, s)| matches!(s, Some(RunStatus::Pending)))
      .map(|(id, _)| *id);
    for task in pending.chain(eligible) {
      if let Err(error) = self.start_run(task.first_run_key(), task).await {
        return self.group_hold(db, group, Some(&rejection_hold(error)));
      }
    }
    self.group_hold(db, group, hold.as_ref())
  }

  fn group_hold(&self, db: &ProjectDb, group: &TaskGroup, hold: Option<&GroupHold>) -> Result<(), Rejection> {
    if db
      .set_task_group_hold(group.key(), hold)
      .map_err(|error| super::failed("cannot record queue hold", &error))?
    {
      self.inner.notifier.send(Event::TaskGroupChanged { key: group.key() });
    }
    Ok(())
  }
}

fn rejection_hold(error: Rejection) -> GroupHold {
  match error {
    Rejection::TaskGroupWaiting { hold } => hold,
    Rejection::NoAgentSlot { .. } => GroupHold::AgentCapacity,
    Rejection::DailyBudgetUsed { .. } => GroupHold::DailyBudget,
    Rejection::Preflight { problems } => GroupHold::Preflight(problems),
    Rejection::LimitNotRaised {
      limit: orchester_core::run::RunLimit::GroupBudget,
    } => GroupHold::GroupCap,
    other => {
      tracing::error!(?other, "reviewed group could not start its next task");
      GroupHold::StartFailed
    }
  }
}
