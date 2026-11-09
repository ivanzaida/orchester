//! Which run waits on which task (`run_waits_on`, `orchester_core::run::RunWaitsOn`): kept when the run starts to
//! wait, one task at a time, and ended once with how it ended. Read as the project's active waits, the active waits on
//! a task (whose merge is an event for each waiting run's task) and a run's latest wait (the task page and the
//! conversation row).

use orchester_core::{
  id::{RunId, TaskId},
  run::{BranchName, RunWaitsOn, WaitedTaskEventKind, WaitsOnEnd},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Params, TransactionBehavior, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A kept wait: since when its run waits, and how it ended once it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptRunWait {
  pub wait: RunWaitsOn,
  pub since: Timestamp,
  pub ended: Option<RunWaitEnded>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunWaitEnded {
  pub at: Timestamp,
  pub end: RunWaitEnd,
}

/// How a wait ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunWaitEnd {
  /// The waited task's work was merged into the waiting run's base branch.
  Merged,
  /// The waited task was canceled.
  Canceled,
  /// The waited task was done without that merge.
  ClosedUnmerged,
  /// The waiting run ended first.
  RunEnded,
  /// The waiting run was answered first, on its own wait card: it went on without the waited task's work.
  Answered,
  /// The person dropped the wait (ORC-784): the run went on without the waited task's work.
  Dropped,
}

impl RunWaitEnd {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Merged => "merged",
      Self::Canceled => "canceled",
      Self::ClosedUnmerged => "closed_unmerged",
      Self::RunEnded => "run_ended",
      Self::Answered => "answered",
      Self::Dropped => "dropped",
    }
  }

  fn parse(value: &str) -> Result<Self, StoreError> {
    match value {
      "merged" => Ok(Self::Merged),
      "canceled" => Ok(Self::Canceled),
      "closed_unmerged" => Ok(Self::ClosedUnmerged),
      "run_ended" => Ok(Self::RunEnded),
      "answered" => Ok(Self::Answered),
      "dropped" => Ok(Self::Dropped),
      other => Err(corrupt("run wait end", other)),
    }
  }
}

impl From<&WaitsOnEnd> for RunWaitEnd {
  fn from(end: &WaitsOnEnd) -> Self {
    match end {
      WaitsOnEnd::Event(event) => match event.kind {
        WaitedTaskEventKind::Merged { .. } => Self::Merged,
        WaitedTaskEventKind::Canceled => Self::Canceled,
        WaitedTaskEventKind::ClosedUnmerged => Self::ClosedUnmerged,
      },
      WaitsOnEnd::Dropped => Self::RunEnded,
    }
  }
}

/// The columns a kept wait is read from, the waiting run's task by its run.
const SELECT: &str = "SELECT w.run_id, r.task_id, w.base_branch, w.waited_task_id, w.since, w.ended_at, w.ended_as
  FROM run_waits_on AS w JOIN runs AS r ON r.id = w.run_id";

impl ProjectDb {
  /// Keeps `wait` from `since`; refused while its run waits on a task already
  /// ([`orchester_core::run::WaitsOnError::AlreadyWaits`]), for a run that is not recorded, for a wait that names
  /// another task as its run's than the run's recorded task ([`StoreError::RunWaitOfAnotherTask`]) and for a waited
  /// task that is not recorded. Nothing is kept then.
  pub fn keep_run_wait(&mut self, wait: &RunWaitsOn, since: Timestamp) -> Result<(), StoreError> {
    // Written at once, so no other writer keeps a wait of the run between the check and the write.
    let transaction = self
      .connection
      .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let recorded: Option<String> = transaction
      .query_row(
        "SELECT task_id FROM runs WHERE id = ?1",
        [wait.run().to_string()],
        |row| row.get(0),
      )
      .optional()?;
    let Some(recorded) = recorded else {
      return Err(StoreError::RunNotFound(wait.run()));
    };
    // The run's task is read back from the run, so the wait keeps only one that is the run's own: `RunWaitsOn` holds
    // that it is not the waited task.
    let recorded: TaskId = parse("task id", &recorded)?;
    if recorded != wait.task() {
      return Err(StoreError::RunWaitOfAnotherTask {
        run: wait.run(),
        recorded,
        named: wait.task(),
      });
    }
    let waited: Option<i64> = transaction
      .query_row(
        "SELECT 1 FROM task_identities WHERE id = ?1",
        [wait.waited().to_string()],
        |row| row.get(0),
      )
      .optional()?;
    if waited.is_none() {
      return Err(StoreError::TaskNotFound(wait.waited()));
    }
    let active = read_waits(
      &transaction,
      "WHERE w.run_id = ?1 AND w.ended_at IS NULL",
      [wait.run().to_string()],
    )?;
    let active: Vec<_> = active.into_iter().map(|kept| kept.wait).collect();
    wait.check_only(&active)?;
    transaction.execute(
      "INSERT INTO run_waits_on (run_id, waited_task_id, base_branch, since) VALUES (?1, ?2, ?3, ?4)",
      params![
        wait.run().to_string(),
        wait.waited().to_string(),
        wait.base().as_str(),
        since.unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// Ends `run`'s active wait at `at` as `end`; whether it had one to end.
  pub fn end_run_wait(&mut self, run: RunId, end: RunWaitEnd, at: Timestamp) -> Result<bool, StoreError> {
    let ended = self.connection.execute(
      "UPDATE run_waits_on SET ended_at = ?2, ended_as = ?3 WHERE run_id = ?1 AND ended_at IS NULL",
      params![run.to_string(), at.unix_millis(), end.as_str()],
    )?;
    Ok(ended == 1)
  }

  /// Drops `run`'s latest wait at `at` (ORC-784): one that is active, or one its waited task ended as canceled or
  /// closed without the merge, which still holds the run for the person; whether it had one to drop.
  pub fn drop_run_wait(&mut self, run: RunId, at: Timestamp) -> Result<bool, StoreError> {
    let dropped = self.connection.execute(
      "UPDATE run_waits_on SET ended_at = ?2, ended_as = 'dropped'
       WHERE id = (SELECT MAX(id) FROM run_waits_on WHERE run_id = ?1)
         AND (ended_at IS NULL OR ended_as IN ('canceled', 'closed_unmerged'))",
      params![run.to_string(), at.unix_millis()],
    )?;
    Ok(dropped == 1)
  }

  /// The project's active waits, in the order they were kept.
  pub fn active_run_waits(&self) -> Result<Vec<KeptRunWait>, StoreError> {
    read_waits(&self.connection, "WHERE w.ended_at IS NULL", [])
  }

  /// The active waits on task `waited`, in the order they were kept.
  pub fn run_waits_on_task(&self, waited: TaskId) -> Result<Vec<KeptRunWait>, StoreError> {
    read_waits(
      &self.connection,
      "WHERE w.waited_task_id = ?1 AND w.ended_at IS NULL",
      [waited.to_string()],
    )
  }

  /// `run`'s latest wait, the last one kept, active or ended; `None` when it never waited on a task.
  pub fn run_wait(&self, run: RunId) -> Result<Option<KeptRunWait>, StoreError> {
    let mut waits = read_waits(&self.connection, "WHERE w.run_id = ?1", [run.to_string()])?;
    Ok(waits.pop())
  }
}

/// The waits `filter`, a fixed SQL condition on them, holds for, in the order they were kept: by the row's
/// increasing id, whatever `since` says (a clock that went back does not reorder them).
fn read_waits(
  connection: &rusqlite::Connection,
  filter: &'static str,
  values: impl Params,
) -> Result<Vec<KeptRunWait>, StoreError> {
  let mut statement = connection.prepare(&format!("{SELECT} {filter} ORDER BY w.id"))?;
  let rows = statement
    .query_map(values, |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, i64>(4)?,
        row.get::<_, Option<i64>>(5)?,
        row.get::<_, Option<String>>(6)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(run, task, base, waited, since, ended_at, ended_as)| {
      let base = BranchName::new(&base).map_err(|error| corrupt("run wait base branch", error))?;
      let wait = RunWaitsOn::restore(
        parse("run id", &run)?,
        parse("task id", &task)?,
        base,
        parse("task id", &waited)?,
      )
      .map_err(|error| corrupt("run wait", error))?;
      let ended = match (ended_at, ended_as) {
        (Some(at), Some(end)) => Some(RunWaitEnded {
          at: Timestamp::from_unix_millis(at),
          end: RunWaitEnd::parse(&end)?,
        }),
        (None, None) => None,
        _ => {
          return Err(corrupt(
            "run wait end",
            "an end without its time, or a time without its end",
          ));
        }
      };
      Ok(KeptRunWait {
        wait,
        since: Timestamp::from_unix_millis(since),
        ended,
      })
    })
    .collect()
}
