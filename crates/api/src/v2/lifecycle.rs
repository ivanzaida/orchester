//! Closing and reopening a team task (decision D4, `docs/architecture/server.md` row 96). The lifecycle change is no
//! command and no route of its own: it is the `workflow` and `blocked_reason` of the task patch
//! (`PATCH /api/v1/projects/{p}/tasks/{t}`, [`TaskPatch`](crate::TaskPatch)), and this module is the rule the server
//! decides it by and the capabilities report it by, so a server, the stand-in and a desktop that previews a change
//! all read one rule.

use orchester_core::{resource::Deliverable, task::organization::WorkflowStatus};

use super::{ArtifactKind, ArtifactRef, DenialReason};
use crate::{TaskChanges, TaskContent};

/// The lifecycle changes a team task has, each an action in the task's capabilities
/// (`Capabilities.actions`, subject `{kind:"task",id}`) under the name [`Self::as_str`] gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LifecycleAction {
  /// An open task to Done: its claim's holder or an Admin.
  Complete,
  /// An open task to Canceled: an Admin only.
  Cancel,
  /// A Done or Canceled task to Backlog or Todo: an Admin only.
  Reopen,
  /// Backlog to Todo and Todo to Backlog: an Admin only.
  Triage,
  /// Setting or clearing the blocked reason: its claim's holder or an Admin.
  Block,
}

impl LifecycleAction {
  pub const ALL: [Self; 5] = [Self::Complete, Self::Cancel, Self::Reopen, Self::Triage, Self::Block];

  /// How the action is named in a task's capabilities.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Complete => "task.complete",
      Self::Cancel => "task.cancel",
      Self::Reopen => "task.reopen",
      Self::Triage => "task.triage",
      Self::Block => "task.block",
    }
  }

  pub(super) fn from_wire(action: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|known| known.as_str() == action)
  }

  /// Whether the holder of the task's effective claim may take it without being an Admin.
  pub const fn holder_may(self) -> bool {
    matches!(self, Self::Complete | Self::Block)
  }

  /// Whether the change releases the task's claim: it is Done or Canceled.
  pub const fn closes(self) -> bool {
    matches!(self, Self::Complete | Self::Cancel)
  }

  /// Whether a task at `status` has the action at all, whoever the caller is.
  pub const fn possible_from(self, status: WorkflowStatus) -> bool {
    match self {
      Self::Complete | Self::Cancel => !status.is_closed(),
      Self::Reopen => status.is_closed(),
      Self::Triage => matches!(status, WorkflowStatus::Backlog | WorkflowStatus::Todo),
      Self::Block => true,
    }
  }

  /// The action a change of the workflow status from `from` to `to` is; `None` for no change, and for the changes D4
  /// does not name (between Done and Canceled, and to or from In progress and In review other than closing), which
  /// stay an Admin's.
  pub const fn of_workflow(from: WorkflowStatus, to: WorkflowStatus) -> Option<Self> {
    use WorkflowStatus::{Backlog, Canceled, Done, InProgress, InReview, Todo};
    match (from, to) {
      (Backlog | Todo | InProgress | InReview, Done) => Some(Self::Complete),
      (Backlog | Todo | InProgress | InReview, Canceled) => Some(Self::Cancel),
      (Done | Canceled, Backlog | Todo) => Some(Self::Reopen),
      (Backlog, Todo) | (Todo, Backlog) => Some(Self::Triage),
      _ => None,
    }
  }

  /// Why `caller` may not take the action on a task at `status`; `None` when they may. The state is told before the
  /// caller: an action the task does not have is `transition_denied` for everybody.
  pub const fn denial(self, status: WorkflowStatus, caller: TaskCaller) -> Option<DenialReason> {
    if !self.possible_from(status) {
      return Some(DenialReason::TransitionDenied);
    }
    if caller.admin || (self.holder_may() && caller.holder) {
      return None;
    }
    Some(if self.holder_may() {
      DenialReason::ClaimRequired
    } else {
      DenialReason::RoleRequired
    })
  }
}

/// Who asks, as far as a task's lifecycle goes. `admin` is a workspace Admin or a server admin. `holder` is that the
/// caller is the person who holds the task's current effective claim, its own or its module's, fresh or stale (a stale
/// claim is the holder's until someone takes it over), from any of their sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskCaller {
  pub admin: bool,
  pub holder: bool,
}

/// What a task patch asks of the task's lifecycle, read against the task as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleRequest {
  actions: Vec<LifecycleAction>,
  other: bool,
}

impl LifecycleRequest {
  /// The lifecycle actions in `changes` and whether it asks for anything else. A change of the workflow status to the
  /// status it has, and of the blocked reason to the reason it has, are no change; every other field the patch names
  /// is a change, equal or not. `actor` is no change. A workflow change that is no action
  /// ([`LifecycleAction::of_workflow`]) is something else, an Admin's.
  pub fn of(current: &TaskContent, changes: &TaskChanges) -> Self {
    let mut actions = Vec::new();
    let mut other = false;
    if let Some(to) = changes.workflow.filter(|to| *to != current.workflow) {
      match LifecycleAction::of_workflow(current.workflow, to) {
        Some(action) => actions.push(action),
        None => other = true,
      }
    }
    if changes
      .blocked_reason
      .as_ref()
      .is_some_and(|reason| reason.as_deref() != current.blocked_reason.as_deref())
    {
      actions.push(LifecycleAction::Block);
    }
    other |= changes.title.is_some()
      || changes.brief.is_some()
      || changes.criteria.is_some()
      || changes.pipeline.is_some()
      || changes.limits.is_some()
      || changes.status.is_some()
      || changes.priority.is_some()
      || changes.tags.is_some()
      || changes.blocking.is_some()
      || changes.module.is_some()
      || changes.release.is_some();
    Self { actions, other }
  }

  pub fn actions(&self) -> &[LifecycleAction] {
    &self.actions
  }

  /// Whether the claim's holder may make the whole patch: it holds only actions a holder may take.
  pub fn holder_may(&self) -> bool {
    !self.other && !self.actions.is_empty() && self.actions.iter().all(|action| action.holder_may())
  }

  /// Whether `caller` may make the whole patch. An Admin may make any; anyone else only a patch of the lifecycle
  /// actions a holder may take, as the holder.
  pub fn permitted(&self, caller: TaskCaller) -> bool {
    caller.admin || (caller.holder && self.holder_may())
  }

  /// Whether the patch closes the task: the server releases its claim with the change.
  pub fn closes(&self) -> bool {
    self.actions.iter().any(|action| action.closes())
  }
}

/// Whether an accepted `succeeded` work report marks its task Done by itself. Work that delivers changes does not: its
/// task is Done when the server accepts its module's delivery into base. Report and outside work has no base merge
/// and is Done on its success; work that delivers changes or a report is, when the run shared no patch artifact
/// (it made no changes).
pub fn completes_on_success(delivery: Deliverable, artifacts: &[ArtifactRef]) -> bool {
  match delivery {
    Deliverable::Changes => false,
    Deliverable::Report | Deliverable::Outside => true,
    Deliverable::ChangesOrReport => !artifacts.iter().any(|artifact| artifact.kind == ArtifactKind::Patch),
  }
}
