//! Moving a task between the Tasks board's columns (ORC-105, `Tasks board · redesign` `J6gQo`), whose columns are the
//! workflow statuses. Backlog means draft and Todo means queued: a queued task whose run never started goes back to
//! the drafts on Backlog, and a complete draft is queued on Todo (an incomplete one is refused). A task whose latest
//! run ended is offered two moves on Todo (ORC-144, the person's decision of 2026-10-28): run again, or move only
//! ([`MOVE_ONLY`]), which sets its workflow status to Todo and starts nothing; its card keeps its last run's outcome
//! (*Run canceled*) until a person runs it again. A task a run of which has not ended (the store's active run,
//! [`TaskRow::run_not_ended`]) moves only to Done or Canceled, which close it after its run is stopped
//! ([`super::close`]); every other column refuses it. In progress and In review only change the workflow status: a
//! drop never starts a run. Done and Canceled close the task whatever its run did. What a drop does is decided here;
//! [`write`] makes the changes that need no runner.

pub mod write;

use orchester_core::{
  run::TaskPhase,
  task::{TaskStatus, organization::WorkflowStatus},
};

use super::{
  close::Closing,
  list::{ReadinessKind, TaskRow},
};
use crate::messages::Message;

/// What dropping a task on a column changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
  /// Its workflow status, nothing else.
  Status(WorkflowStatus),
  /// The complete draft is queued, and set to Todo.
  Queue,
  /// The queued task whose run never started goes back to the drafts, and to Backlog.
  ReturnToDraft,
  /// The task whose latest run ended goes to Todo as the person chooses: it runs again, or only moves
  /// ([`MOVE_ONLY`]).
  RunAgain,
  /// The task is closed: Done or Canceled, its run stopped first when it has not ended.
  Close(Closing),
}

/// What a task whose latest run ended takes on Todo instead of running again: its workflow status becomes Todo and no
/// run starts, nor anything queued that would start one. It runs only when a person runs it again.
pub const MOVE_ONLY: Move = Move::Status(WorkflowStatus::Todo);

/// Why a task does not go to a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
  /// A run of it has not ended: only Done and Canceled, which stop it, take it.
  RunNotEnded,
  /// A draft that misses details cannot be queued.
  Incomplete,
  /// It has a run, so it no longer goes back to the drafts.
  HasRun,
}

/// What a drop of a task on a column comes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
  /// It is in that column already: nothing changes.
  Stay,
  Move(Move),
  Refused(Refusal),
}

/// What dropping `row`'s task on the column of `to` does.
pub fn decide(row: &TaskRow, to: WorkflowStatus) -> Decision {
  if row.organization.status() == to {
    return Decision::Stay;
  }
  match to {
    WorkflowStatus::Done => return Decision::Move(Move::Close(Closing::Done)),
    WorkflowStatus::Canceled => return Decision::Move(Move::Close(Closing::Canceled)),
    _ if row.run_not_ended => return Decision::Refused(Refusal::RunNotEnded),
    _ => {}
  }
  match (to, row.status, row.has_run) {
    (WorkflowStatus::Backlog, _, true) => Decision::Refused(Refusal::HasRun),
    (WorkflowStatus::Backlog, TaskStatus::Queued, false) => Decision::Move(Move::ReturnToDraft),
    (WorkflowStatus::Todo, TaskStatus::Draft, false) if row.readiness == ReadinessKind::Missing => {
      Decision::Refused(Refusal::Incomplete)
    }
    (WorkflowStatus::Todo, TaskStatus::Draft, false) => Decision::Move(Move::Queue),
    (WorkflowStatus::Todo, _, true) => Decision::Move(Move::RunAgain),
    (status, ..) => Decision::Move(Move::Status(status)),
  }
}

/// Whether closing `row`'s task leaves work that waits to be merged: its run passed every step and committed work
/// nobody merged yet. Done asks first, since the task closes without merging and its branch stays.
pub fn leaves_unmerged(row: &TaskRow) -> bool {
  row.phase == Some(TaskPhase::Succeeded) && !row.nothing_to_merge
}

/// Why deleting the chosen tasks was not taken from input that is not a person's, such as an MCP client's: only a
/// person deletes tasks.
pub fn delete_person_only() -> Message {
  Message::new("tasks.board.deletePersonOnly")
}
