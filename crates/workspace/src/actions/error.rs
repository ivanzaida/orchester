use orchester_api::v2::DecisionId;
use orchester_core::id::{OperationId, TaskId};
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
  /// The action sets something whose owner is an open team-authority decision (a task's pipeline, limits or workflow
  /// status, D4/D6): no team command sets it.
  #[error("this change waits for team-authority decision {0:?}")]
  DecisionRequired(DecisionId),
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
