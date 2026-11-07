//! Letting go of the claim of a start that will never run. A start the server made no work for, or whose bundle
//! can never become a run here, is settled for good; nothing renews its claim, so holding it only keeps teammates
//! from the task until the claim goes stale (`ClaimStalenessThreshold`).
//!
//! The contract has `claim.release` and no way to report a work failed without a run: `work.report` is a run's own
//! (`run_id`, `sequence`, `ExecutionFailure` has no start failure), and this computer makes no run to report for.
//! Releasing the claim is what the server's `claim.release` gives back; the work is not reported.

use orchester_api::v2::AuthorityRef;
use orchester_core::id::{ClaimId, OperationId};

use super::{TeamWork, TeamWorkError, start::KeptStart};
use crate::actions::{Action, ActionOutcome, ClaimAction, Intent, TeamOutcome};

/// Where the release of a claim stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Released {
  /// The server let go of the claim, or has none of this computer's to let go of.
  Done,
  /// The server cannot be reached, or refused for now: nothing changed, and the claim is still held.
  Waits,
}

impl<R> TeamWork<R> {
  /// Releases the task claim `authority` names for the start `operation`, unless a team run or another open start here
  /// goes on under the same claim: letting go of it would fence their work. A release the server refused for good
  /// (`claim_lost`, a claim already released, …) leaves nothing to let go of. One whose answer was lost is sent again
  /// when the start goes on; the server then answers the claim is not this computer's, which is the same end.
  pub(super) async fn release_claim(
    &self,
    operation: OperationId,
    authority: AuthorityRef,
  ) -> Result<Released, TeamWorkError> {
    if self.claim_in_use(operation, authority.claim_id)? {
      return Ok(Released::Done);
    }
    let authority = AuthorityRef {
      work_id: None,
      ..authority
    };
    let intent = Intent::new(Action::Claim(ClaimAction::Release { authority }));
    let ActionOutcome::Team(outcome) = self.dispatcher.dispatch(&intent).await? else {
      return Err(TeamWorkError::NotAWork(operation));
    };
    Ok(match outcome {
      TeamOutcome::Rejected { retryable: true, .. } | TeamOutcome::AcceptanceUnknown { .. } => Released::Waits,
      // A committed release whose local effect failed is made again by the operation's own recovery.
      TeamOutcome::Rejected { .. }
      | TeamOutcome::AcceptedPending { .. }
      | TeamOutcome::Completed(_)
      | TeamOutcome::LocalEffectFailed { .. } => Released::Done,
    })
  }

  /// Whether a team run, or an open start other than `operation`, works under `claim` on this computer.
  fn claim_in_use(&self, operation: OperationId, claim: ClaimId) -> Result<bool, TeamWorkError> {
    let db = self.db()?;
    if db.team_runs()?.iter().any(|run| run.claim == claim) {
      return Ok(true);
    }
    // A start whose request cannot be read is carried on to say so, not here.
    Ok(db.open_team_starts()?.iter().any(|start| {
      start.operation != operation
        && serde_json::from_str::<KeptStart>(&start.request).is_ok_and(|kept| kept.authority.claim_id == claim)
    }))
  }
}
