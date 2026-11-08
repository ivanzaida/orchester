use orchester_api::v2::{BundleFileError, EXECUTION_BUNDLE_SCHEMA};
use orchester_core::{
  blob::ContentHash,
  id::{OperationId, RunId},
  integration::GitError,
  run::inputs::InputsError,
};
use orchester_exec::local_git::LocalGitError;
use orchester_store::{RunStoreError, StoreError};

use crate::{actions::ActionError, readiness::MissingPart};

impl TeamWorkError {
  /// Whether trying again changes nothing: the recorded start itself cannot be read, which only a newer Orchester or a
  /// person can mend. Every other error may pass (the database was busy, the server answered strangely).
  pub const fn is_permanent(&self) -> bool {
    matches!(self, Self::Unreadable(_))
  }
}

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
  /// The server built the start's bundle in another schema than this computer reads, so it can never be read here.
  #[error("the bundle is built in schema {built}, and this computer reads schema {EXECUTION_BUNDLE_SCHEMA}")]
  BundleSchema { built: u32 },
  /// The server answered JSON for the start's bundle that is not an execution bundle of the schema this computer reads.
  #[error("the bundle the server answered is not one this computer can read")]
  BundleUnreadable,
  /// A bundle file the server answered is not the file the bundle pins.
  #[error(transparent)]
  BundleFile(#[from] BundleFileError),
  /// The accepted bundle with the caller's choices does not make a run's inputs.
  #[error("the accepted bundle does not make the run's inputs: {0}")]
  Inputs(#[from] InputsError),
  /// The versions the bundle pins need something this computer lacks, though the pipeline was ready here when the start
  /// asked: the team server pinned newer versions than the cached ones.
  #[error("the pipeline {pipeline} is not ready on this computer for the versions the bundle pins")]
  NotReadyHere {
    pipeline: String,
    missing: Vec<MissingPart>,
  },
  /// The recorded request or receipt of the start cannot be read back.
  #[error("the record of team start {0} cannot be read")]
  Unreadable(OperationId),
  /// The start a person dismissed is not the newest of its task any more: a later one replaced what the page showed.
  #[error("team start {0} is no longer the newest start of its task")]
  StartSuperseded(OperationId),
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
  /// The checkout cannot be held for the delivery's merge, and not because another merge holds it: the merge waits
  /// at its stage until the fault is fixed.
  #[error("the checkout cannot be held for the delivery's merge: {0}")]
  DeliveryHold(LocalGitError),
}
