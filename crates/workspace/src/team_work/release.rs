//! Letting go of the claim of a start that will never run. A start the server made no work for, or whose bundle
//! can never become a run here, is settled for good; nothing renews its claim, so holding it only keeps teammates
//! from the task until the claim goes stale (`ClaimStalenessThreshold`).
//!
//! The contract has `claim.release` and no way to report a work failed without a run: `work.report` is a run's own
//! (`run_id`, `sequence`, `ExecutionFailure` has no start failure), and this computer makes no run to report for.
//! Releasing the claim is what the server's `claim.release` gives back; the work is not reported.

use orchester_api::v2::{AuthorityErrorCode, AuthorityRef};
use orchester_core::{
  id::{ClaimId, OperationId},
  run::ExecutionOutcome,
  store::RunStore,
};
use orchester_store::{ProjectDb, StoreError, TeamRun, WorkReportState};
use serde::{Deserialize, Serialize};

use super::{TeamWork, TeamWorkError, start::KeptStart};
use crate::actions::{Action, ActionOutcome, ClaimAction, Intent, Refusal, TeamOutcome};

/// What became of the claim of a start that settled without a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimEnd {
  /// The server let go of the claim, or has none of this computer's to let go of any more.
  Released,
  /// A team run here that still needs the claim, or another open start, goes on under it, so it was not let go of.
  Kept,
  /// The server refused the release for good (`forbidden`, `membership_revoked`, …): the claim may still be held, and
  /// the person's next try is up to the server.
  Held,
}

/// Where the release of a claim stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Released {
  /// The release is settled: how the claim ended.
  Ended(ClaimEnd),
  /// The server cannot be reached, or refused for now: nothing changed, and the claim is still held.
  Waits,
}

impl<R> TeamWork<R> {
  /// Releases the task claim `authority` names for the start `operation`, unless a team run that still works, reports
  /// or can be recovered, or another open start here, goes on under the same claim: letting go of it would fence their
  /// work ([`ClaimEnd::Kept`]). A claim the server says is gone (`claim_lost`) leaves nothing to let go of; any other
  /// refusal for good is [`ClaimEnd::Held`]. One whose answer was lost is sent again when the start goes on; the server
  /// then answers the claim is not this computer's, which is the same end.
  pub(super) async fn release_claim(
    &self,
    operation: OperationId,
    authority: AuthorityRef,
  ) -> Result<Released, TeamWorkError> {
    if self.claim_in_use(operation, authority.claim_id)? {
      return Ok(Released::Ended(ClaimEnd::Kept));
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
      TeamOutcome::Rejected {
        refusal: Refusal::Command(error),
        ..
      } if error.code() == AuthorityErrorCode::ClaimLost => Released::Ended(ClaimEnd::Released),
      TeamOutcome::Rejected { .. } => Released::Ended(ClaimEnd::Held),
      // A committed release whose local effect failed is made again by the operation's own recovery.
      TeamOutcome::AcceptedPending { .. } | TeamOutcome::Completed(_) | TeamOutcome::LocalEffectFailed { .. } => {
        Released::Ended(ClaimEnd::Released)
      }
    })
  }

  /// Whether a team run that has not ended for good here, or an open start other than `operation`, works under `claim`.
  fn claim_in_use(&self, operation: OperationId, claim: ClaimId) -> Result<bool, TeamWorkError> {
    let db = self.db()?;
    for run in db.team_runs()?.iter().filter(|run| run.claim == claim) {
      if works_or_reports(&db, run)? {
        return Ok(true);
      }
    }
    // A start whose request cannot be read is carried on to say so, not here.
    Ok(db.open_team_starts()?.iter().any(|start| {
      start.operation != operation
        && serde_json::from_str::<KeptStart>(&start.request).is_ok_and(|kept| kept.authority.claim_id == claim)
    }))
  }
}

/// Whether `run` still needs the claim: it executes, has an outcome the server was not told yet, or failed in a way a
/// person can recover it from here. A recovered run goes on under its claim, which a release would have taken from it
/// (`claim_lost`), and a start that was refused made no newer work to fence it; so the claim is kept while a recovery
/// is possible, and is let go of only once the run is fenced or ended for good. A run the server fenced reports nothing
/// more, and one that ended with every report answered needs the claim no more.
fn works_or_reports(db: &ProjectDb, run: &TeamRun) -> Result<bool, TeamWorkError> {
  if run.fence.is_some() {
    return Ok(false);
  }
  let status = db.run(run.run)?.ok_or(StoreError::RunNotFound(run.run))?;
  if !status.status().is_terminal() || status.recovery_step(None).is_some() {
    return Ok(true);
  }
  // The outcome is queued before it is sent: it is unsent while the last one queued is another, or a report waits.
  let outcome = ExecutionOutcome::of_status(status.status())
    .map(|outcome| serde_json::to_string(&outcome).expect("an outcome serializes"));
  let waiting = db
    .work_reports(run.run)?
    .iter()
    .any(|report| report.state == WorkReportState::Pending);
  Ok(waiting || outcome != run.last_outcome)
}
