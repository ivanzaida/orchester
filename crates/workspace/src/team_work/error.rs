use orchester_api::v2::BundleFileError;
use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{OperationId, ResourceId, RunId},
  integration::GitError,
  run::inputs::InputsError,
};
use orchester_store::{RunStoreError, StoreError};

use crate::actions::ActionError;

/// Why a team start, its reports, its heartbeat or a module delivery could not go on. A server that cannot be reached
/// or refuses for now is no error: the outcome says so, and the work goes on from where it stands.
#[derive(Debug, thiserror::Error)]
pub enum TeamWorkError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  RunStore(#[from] RunStoreError),
  #[error(transparent)]
  Action(#[from] ActionError),
  /// The start's work action names a module's work, which waits on its own bundle (D3).
  #[error("only a task's work starts here")]
  NotATask,
  /// The bundle the server answered is not the one the receipt names, or not built for this start.
  #[error("the bundle read for {expected} is not the start's: {reason}")]
  BundleMismatch {
    expected: ContentHash,
    reason: &'static str,
  },
  /// A bundle file the server answered is not the file the bundle pins.
  #[error(transparent)]
  BundleFile(#[from] BundleFileError),
  /// The accepted bundle with the caller's choices does not make a run's inputs.
  #[error("the accepted bundle does not make the run's inputs: {0}")]
  Inputs(#[from] InputsError),
  /// A pinned profile is bound to a connection here that does not run the CLI the bundle binds it to.
  #[error("agent profile {resource} is bound to a connection that is not {cli:?}")]
  BindingCli { resource: ResourceId, cli: AgentProvider },
  /// The recorded request or receipt of the start cannot be read back.
  #[error("the record of team start {0} cannot be read")]
  Unreadable(OperationId),
  /// The server answered the start's operation with something other than a work.
  #[error("the server answered team start {0} with no work")]
  NotAWork(OperationId),
  /// A report the run queued, or the work the server accepted of it, cannot be read back.
  #[error("the reports of team run {0} cannot be read")]
  UnreadableRun(RunId),
  /// The delivery action asks for an Admin's authorization: only the module holder's request is delivered here.
  #[error("only a delivery request is delivered here")]
  NotADeliveryRequest,
  /// The server answered the delivery's request or report with no delivery, or without the `deliver_module` effect of
  /// exactly the requested branch and commits: nothing is merged on it.
  #[error("the server answered team delivery {0} with no delivery this computer may merge")]
  NotADelivery(OperationId),
  /// The recorded request, delivery, refusal or report of the delivery cannot be read back.
  #[error("the record of team delivery {0} cannot be read")]
  UnreadableDelivery(OperationId),
  /// Git could not tell whether the delivery was merged.
  #[error(transparent)]
  DeliveryGit(GitError),
}
