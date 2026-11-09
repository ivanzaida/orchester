//! A team task's lifecycle from the screens (decision D4, `docs/cloud-api.md` "Team authority task lifecycle"): Mark as
//! done, Cancel task, Reopen to Todo or Backlog, a move between Backlog and Todo, Mark as blocked and Clear blocked.
//! Each is the task's patch on the revision the screen read, which the team server allows by the person's claim and
//! role; the screens offer one only while the task's capabilities list it ([`super::pages::Lifecycle`]).

use orchester_core::{
  sharing::TaskRevision,
  task::{
    Task,
    organization::{OrganizationError, TaskOrganization, WorkflowStatus},
  },
};
use orchester_workspace::actions::{Action, TaskAction};

/// What a person changes of a team task's lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
  Done,
  Cancel,
  /// Reopens a closed task, or moves an open one between Backlog and Todo, to this status.
  MoveTo(WorkflowStatus),
  /// Sets the blocked reason, which is not blank.
  Block(String),
  ClearBlocked,
}

impl Change {
  /// The action that makes this change of `task` as it was read at `read`; `Err` for a blank blocked reason.
  pub fn action(&self, task: &Task, read: TaskRevision) -> Result<Action, OrganizationError> {
    let base = task.organization().clone();
    let edited = match self {
      Self::Done => base.clone().with_status(WorkflowStatus::Done),
      Self::Cancel => base.clone().with_status(WorkflowStatus::Canceled),
      Self::MoveTo(status) => base.clone().with_status(*status),
      Self::Block(reason) => blocked(&base, Some(reason.clone()))?,
      Self::ClearBlocked => blocked(&base, None)?,
    };
    Ok(Action::Task(TaskAction::Organize {
      task: task.id(),
      read,
      base,
      edited,
    }))
  }
}

/// `base` with `reason` as its blocked reason.
fn blocked(base: &TaskOrganization, reason: Option<String>) -> Result<TaskOrganization, OrganizationError> {
  TaskOrganization::new(
    base.status(),
    base.priority(),
    reason,
    base.dependencies().to_vec(),
    base.tags().to_vec(),
  )
}
