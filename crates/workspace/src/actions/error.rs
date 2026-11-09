use orchester_api::v2::DecisionId;
use orchester_core::{
  id::{OperationId, ResourceId, TaskId},
  task::organization::WorkflowStatus,
};
use orchester_store::StoreError;

use crate::tasks::writer::WriteError;

/// Why an action was not carried out. Each is decided before anything is sent to a team server, except
/// [`Self::ReceiptMismatch`], which is the server's answer to another operation.
#[derive(Debug, thiserror::Error)]
pub enum ActionError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The local project's change was refused, as it is from its screens.
  #[error(transparent)]
  Write(#[from] WriteError),
  /// Claims, assignment, the workspace policy, delivery and module branches are a team project's only.
  #[error("this action is a team project's only")]
  TeamOnly,
  /// The project is a team's, and no session of its server is signed in: nothing is changed on this computer instead.
  #[error("the project's team server is not signed in")]
  NoTeamSession,
  /// The session given is another server's, or another server project's, than the one the project is linked to.
  #[error("the session is another server project's")]
  OtherProject,
  /// The session the dispatcher was made in ended: the person signed out or switched account or server.
  #[error("the team session changed")]
  SessionChanged,
  /// A task kept on this computer only in a team project: it is read-only, and never the team's.
  #[error("task {0} is this computer's, not the team's")]
  LocalTask(TaskId),
  #[error("the action changes nothing")]
  NothingToChange,
  /// The action sets something whose owner is an open team-authority decision (a task's limits, D6): no team command
  /// sets it.
  #[error("this change waits for team-authority decision {0:?}")]
  DecisionRequired(DecisionId),
  /// A change of a team task's workflow status that the lifecycle (decision D4) does not offer: only Done, Canceled,
  /// reopening to Backlog or Todo, and Backlog to Todo and back are a person's to make; In progress and In review
  /// follow the task's work.
  #[error("a team task cannot be moved from {from:?} to {to:?}")]
  WorkflowNotOffered { from: WorkflowStatus, to: WorkflowStatus },
  /// A new team task is made in the default workflow status; later statuses are the lifecycle's, on the task's patch
  /// (decision D4), so a create does not carry one.
  #[error("a new team task starts in {0:?}")]
  StatusOnCreate(WorkflowStatus),
  /// A team task names only one of the team's pipelines, which admins create and publish; this one is none of them
  /// here, e.g. a person's own local pipeline (`docs/architecture/server.md` row 80). Nothing was sent.
  #[error("pipeline {0} is not one of the team's pipelines")]
  NotTeamPipeline(ResourceId),
  /// The operation id was used for another action: one id is one intent.
  #[error("operation {0} was used for another action")]
  OperationMismatch(OperationId),
  /// The operation was recorded for another server project than the one the project is linked to now, e.g. before it
  /// was made local and joined elsewhere: it is neither looked up nor sent here.
  #[error("operation {0} was recorded for another server project")]
  OperationOfOtherProject(OperationId),
  /// The server answered with another operation's receipt; nothing of it was taken.
  #[error("the server answered operation {0} with another operation's receipt")]
  ReceiptMismatch(OperationId),
  /// The recorded request, receipt or refusal of the operation cannot be read back.
  #[error("the record of operation {0} cannot be read")]
  Unreadable(OperationId),
}

impl ActionError {
  /// `error` of an operation's record: the store's mismatch is the dispatcher's own.
  pub(super) fn of_record(error: StoreError) -> Self {
    match error {
      StoreError::OperationMismatch(operation) => Self::OperationMismatch(operation),
      error => Self::Store(error),
    }
  }
}
