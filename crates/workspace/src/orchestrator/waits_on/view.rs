//! How a run's wait on a task stands, as its row in the leading conversation and the waiting task's page show it
//! (ORC-784; `docs/design/orchestrator-watch.md` decisions 9–11, `rlnEZ` in `Orchestrator watch · redesign`): it
//! waits; it went on once the waited task was merged into the run's base branch; the waited task was canceled or closed
//! without that merge and the run still waits for the person, so nothing will end the wait; the person dropped it; or
//! it ended otherwise (the run was answered first, or it ended).

use orchester_core::{
  id::{RunId, TaskId},
  run::Run,
  store::{ConversationStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, RunWaitEnd, StoreError};

use super::follow::{answered_since, waits_for_person};

/// A run's latest wait on a task, with what its row and the task page name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunWaitView {
  pub run: RunId,
  /// The run's number among its task's runs.
  pub number: u32,
  /// The waiting run's task, by its key as people read it (`KON-41`).
  pub task: String,
  /// The waited task, by its key (`KON-52`) and its title; the title is empty once the project no longer keeps it.
  pub waited: String,
  pub waited_title: String,
  /// The branch the waited task's work is to be merged into: the waiting run's base branch.
  pub base: String,
  pub since: Timestamp,
  pub state: WaitState,
}

/// Where a wait stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitState {
  /// The run waits on the task.
  Waiting,
  /// The waited task was merged into the run's base branch at `at`: the run went on.
  Continued { at: Timestamp },
  /// The waited task ended at `at` without that merge (`how`), and the run still waits for the person: nothing will
  /// end the wait but them.
  Unmet { at: Timestamp, how: Unmet },
  /// The person dropped the wait at `at`, and the run went on without the waited task's work.
  Dropped { at: Timestamp },
  /// It ended at `at` otherwise: the run was answered first, or it ended.
  Over { at: Timestamp },
}

/// How a waited task ended without its merge into the waiting run's base branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmet {
  Canceled,
  ClosedUnmerged,
}

impl WaitState {
  /// Whether the run still waits on the task, so the person may drop the wait.
  pub const fn holds(self) -> bool {
    matches!(self, Self::Waiting | Self::Unmet { .. })
  }
}

/// How `run`'s latest wait in `db` stands; `None` when it never waited on a task. Blocks on SQLite.
pub fn read(db: &ProjectDb, run: &Run) -> Result<Option<RunWaitView>, StoreError> {
  let Some(kept) = db.run_wait(run.id())? else {
    return Ok(None);
  };
  let state = match kept.ended {
    None => WaitState::Waiting,
    Some(ended) => {
      let at = ended.at;
      match ended.end {
        RunWaitEnd::Merged => WaitState::Continued { at },
        RunWaitEnd::Dropped => WaitState::Dropped { at },
        RunWaitEnd::Canceled | RunWaitEnd::ClosedUnmerged
          if waits_for_person(run) && !answered_since(&db.messages(run.task())?, run.id(), at) =>
        {
          let how = if ended.end == RunWaitEnd::Canceled {
            Unmet::Canceled
          } else {
            Unmet::ClosedUnmerged
          };
          WaitState::Unmet { at, how }
        }
        RunWaitEnd::Canceled | RunWaitEnd::ClosedUnmerged | RunWaitEnd::RunEnded | RunWaitEnd::Answered => {
          WaitState::Over { at }
        }
      }
    }
  };
  let (task, _) = named(db, run.task())?;
  let (waited, waited_title) = named(db, kept.wait.waited())?;
  Ok(Some(RunWaitView {
    run: run.id(),
    number: run.number().get(),
    task,
    waited,
    waited_title,
    base: kept.wait.base().as_str().to_owned(),
    since: kept.since,
    state,
  }))
}

/// Task `task`'s key and title; its id and no title once the project no longer keeps it.
fn named(db: &ProjectDb, task: TaskId) -> Result<(String, String), StoreError> {
  Ok(db.task(task)?.map_or_else(
    || (task.to_string(), String::new()),
    |task| (task.key().to_string(), task.details().title.as_str().to_owned()),
  ))
}
