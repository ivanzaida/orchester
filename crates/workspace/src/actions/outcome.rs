//! What became of an action (`docs/architecture/team-authority-plan.md` §4, "Results and failure recovery"): a local
//! action's result, or one of a team action's explicit outcomes. A lost response is never read as a refusal, and an
//! accepted change stays accepted whatever happens on this computer afterwards: nothing undoes it.

use orchester_api::{
  ErrorCode, SharedModule, SharedTag, SharedTask, SharingErrorDetails,
  client::AuthorityCallError,
  releases::ReleaseVersion,
  v2::{AuthorityError, CommandResult, LocalEffect, ModuleRead, TaskRead, TeamCommandValue, WorkspacePolicy},
};
use orchester_core::{
  id::OperationId,
  module::Module,
  task::{TaskUpdate, organization::Tag},
};
use orchester_store::StoreError;

#[derive(Debug)]
pub enum ActionOutcome {
  Local(LocalResult),
  Team(TeamOutcome),
}

/// A local action's result, as the project's writer gives it.
#[derive(Debug)]
pub enum LocalResult {
  /// A new task, or an edit of one: saved, or refused for a change made meanwhile with the task as it is now.
  Task(Box<TaskUpdate>),
  /// Whether an organization change or a move changed the task.
  Changed(bool),
  Module(Module),
  Tag(Tag),
  /// A tag renamed, merged or deleted.
  Tags,
}

/// A committed command's receipt: its value, with the local effects the server allowed, and the stream cursor just
/// after its events.
pub type Receipt = CommandResult<TeamCommandValue>;

#[derive(Debug)]
pub enum TeamOutcome {
  /// The server refused it, or it was superseded. A `retryable` refusal (signed out, rate limited, unavailable, an
  /// unsupported client) left nothing changed: a command's operation stays open, to send again under the same id. Any
  /// other is final, and a command's is kept. A revision conflict comes with the subject as the server holds it now.
  Rejected {
    operation: OperationId,
    refusal: Refusal,
    retryable: bool,
    current: Option<Current>,
  },
  /// A command committed; `pending` are local effects of its receipt this computer has still to carry out (starting
  /// work: P4). The operation stays open until they are.
  AcceptedPending {
    receipt: Box<Receipt>,
    pending: Vec<LocalEffect>,
  },
  /// Accepted, and every local effect of a command's receipt happened.
  Completed(Accepted),
  /// A command committed, and a local effect of its receipt failed; the command stays committed, and the effect is
  /// made again when the operation is recovered.
  LocalEffectFailed { receipt: Box<Receipt>, error: StoreError },
  /// The response was lost or unreadable, and what the server made of it could not be learned. A command's operation
  /// stays open: recover it, or send the same intent again, which looks it up before sending it under the same id. A
  /// definition edit is sent again with the same intent, which reads the subject before it is taken as refused.
  AcceptanceUnknown { operation: OperationId },
}

/// What the team server accepted.
#[derive(Debug)]
pub enum Accepted {
  /// A command, with its receipt.
  Command(Box<Receipt>),
  /// A definition edit, as its route or a read after a lost response shows it.
  Definition(Definition),
}

/// A definition as the team server holds it after an accepted edit.
#[derive(Debug)]
pub enum Definition {
  Task(Box<SharedTask>),
  Module(Box<SharedModule>),
  Tag(SharedTag),
  /// A tag merged or deleted: the project's tags after it, and the tasks whose tags it changed; `None` when the answer
  /// naming them was lost and the tags were read again instead.
  Tags {
    tags: Vec<SharedTag>,
    tasks: Option<Vec<SharedTask>>,
  },
  Policy(WorkspacePolicy),
}

/// Why the team server refused the pipeline a task names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineRefusal {
  /// The project has no pipeline of that id.
  NotFound,
  /// The team's pipeline of that id is archived.
  Archived,
}

impl PipelineRefusal {
  /// The refusal a `422 invalid`'s `reason` names; `None` for any other reason.
  pub fn of_reason(reason: &str) -> Option<Self> {
    match reason {
      "pipeline_not_found" => Some(Self::NotFound),
      "pipeline_archived" => Some(Self::Archived),
      _ => None,
    }
  }
}

/// Why the team server refused to close, reopen or block a task (decision D4, `docs/cloud-api.md` "Team authority task
/// lifecycle"). Nothing was changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleRefusal {
  /// `409 not_holder`: the change is the holder of the task's claim's, and the caller is neither them nor an Admin,
  /// e.g. the claim was taken over or released since the task was read.
  NotHolder,
  /// `422 organization`, `stop_run_first`: the task's work runs; a close waits for it to be stopped.
  StopRunFirst,
  /// `403 not_admin`: the change (cancel, reopen, move between Backlog and Todo) is an Admin's alone, and the caller is
  /// a member.
  NotAdmin,
  /// `403 not_admin` of a patch that also changes the task's other fields: the refusal does not say which of the two
  /// the server objected to, so the words cover both.
  NotAdminWithDetails,
  /// `422 organization`, `missing_reason`: a blocked reason is not empty after trimming.
  MissingReason,
}

impl LifecycleRefusal {
  /// The refusal a `422 organization`'s `reason` names; `None` for any other reason.
  pub fn of_reason(reason: &str) -> Option<Self> {
    match reason {
      "stop_run_first" => Some(Self::StopRunFirst),
      "missing_reason" => Some(Self::MissingReason),
      _ => None,
    }
  }
}

/// Why the team server did not take an action.
#[derive(Debug)]
pub enum Refusal {
  /// A command's refusal, as the server keeps it under the operation.
  Command(AuthorityError),
  /// `426 unsupported_contract`: the server accepts desktops of `required` and later only. Nothing was kept, and the
  /// same command after the update is safe.
  NeedsVersion { required: ReleaseVersion },
  /// A definition route's refusal: its HTTP status and code, e.g. `403 not_admin` for a member, or
  /// `409 revision_conflict`.
  Route { status: u16, code: ErrorCode },
  /// A task's definition route refused the pipeline it names: `422 invalid` with the reason `pipeline_not_found` or
  /// `pipeline_archived` (`docs/architecture/server.md` row 80).
  TaskPipeline(PipelineRefusal),
  /// A task's definition route refused to close, reopen or block it (decision D4).
  TaskLifecycle(LifecycleRefusal),
  /// `PATCH …/policy`'s `409 policy_revision_conflict`.
  PolicyRevisionConflict,
  /// A definition edit's response was lost, and the subject read again holds neither the change nor the revision it
  /// was decided on: it changed meanwhile.
  Superseded,
}

impl Refusal {
  /// The refusal a task route's answer is: a pipeline the server refused (`422 invalid` with its reason), a lifecycle
  /// change it refused (decision D4: `422 organization` with `stop_run_first` or `missing_reason`, `409 not_holder`),
  /// or its status and code.
  pub fn of_task_route(status: u16, code: ErrorCode, details: Option<&SharingErrorDetails>) -> Self {
    match (code, details) {
      (ErrorCode::Invalid, Some(SharingErrorDetails::Reason { reason })) => {
        PipelineRefusal::of_reason(reason).map(Self::TaskPipeline)
      }
      (ErrorCode::Organization, Some(SharingErrorDetails::Reason { reason })) => {
        LifecycleRefusal::of_reason(reason).map(Self::TaskLifecycle)
      }
      (ErrorCode::NotHolder, _) => Some(Self::TaskLifecycle(LifecycleRefusal::NotHolder)),
      _ => None,
    }
    .unwrap_or(Self::Route { status, code })
  }
}

/// The subject of a refused change as the server holds it now.
#[derive(Debug)]
pub enum Current {
  /// A task as its definition route shows it.
  Task(Box<SharedTask>),
  /// A task as the workspace reads show it, after a command's conflict.
  TaskRead(Box<TaskRead>),
  /// A module as its definition route shows it.
  Module(Box<SharedModule>),
  /// A module as the workspace reads show it, after a command's conflict.
  ModuleRead(Box<ModuleRead>),
  Policy(WorkspacePolicy),
  /// The project's tags, when the tag an edit names is gone.
  Tags(Vec<SharedTag>),
  /// It could not be read; the refusal stands without it.
  NotRead(AuthorityCallError),
}
