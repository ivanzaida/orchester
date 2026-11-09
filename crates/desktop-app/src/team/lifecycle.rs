//! A team task's lifecycle from the screens (decision D4, `docs/cloud-api.md` "Team authority task lifecycle"): Mark as
//! done, Cancel task, Reopen to Todo or Backlog, a move between Backlog and Todo, Mark as blocked and Clear blocked.
//! Each is the task's patch on the revision the screen read, which the team server allows by the person's claim and
//! role; the screens offer one only while the task's capabilities list it ([`Offered`], [`super::pages::Lifecycle`]).

use orchester_core::{
  id::TaskId,
  sharing::TaskRevision,
  task::organization::{OrganizationError, TaskOrganization, WorkflowStatus},
};
use orchester_workspace::actions::{Action, TaskAction};

use super::pages::Lifecycle;

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
  /// The action that makes this change of `task`, whose organization read at `read` is `base`; `Err` for a blank
  /// blocked reason.
  pub fn action(&self, task: TaskId, base: &TaskOrganization, read: TaskRevision) -> Result<Action, OrganizationError> {
    let edited = match self {
      Self::Done => base.clone().with_status(WorkflowStatus::Done),
      Self::Cancel => base.clone().with_status(WorkflowStatus::Canceled),
      Self::MoveTo(status) => base.clone().with_status(*status),
      Self::Block(reason) => blocked(base, Some(reason.clone()))?,
      Self::ClearBlocked => blocked(base, None)?,
    };
    Ok(Action::Task(TaskAction::Organize {
      task,
      read,
      base: base.clone(),
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

/// What a listed task offers the person: the lifecycle actions its capabilities allow, the revision they are made on
/// and the name of the person who holds its claim, for the line saying who else can.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offered {
  pub abilities: Lifecycle,
  pub revision: TaskRevision,
  pub holder: Option<String>,
}
