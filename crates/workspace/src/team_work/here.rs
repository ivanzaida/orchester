//! What this computer holds of a team project's claims and runs, for the screens (P6): the claims it took and has not
//! released, from the receipts of its own claim commands and the claims its team runs started under, and why a team
//! run reports nothing more once the server said its authority is gone. A claim is a computer's, not an account's: the
//! server's reads name its holder only, so whether this computer holds it is read here.

use std::path::Path;

use orchester_api::v2::TeamCommandValue;
use orchester_core::id::{ClaimId, RunId};
use orchester_store::{ProjectDb, StoreError, TeamFence};

use crate::actions::Receipt;

/// A team project's claims and fenced runs on this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamHere {
  /// The claims this computer took and still holds, as far as it knows: the server may have ended one since.
  pub claims: Vec<ClaimId>,
  /// The team runs the server fenced, with why.
  pub fences: Vec<(RunId, TeamFence)>,
}

impl TeamHere {
  /// Whether this computer took `claim` and has not released it.
  pub fn holds(&self, claim: ClaimId) -> bool {
    self.claims.contains(&claim)
  }

  /// Why the server fenced team run `run`; `None` while it reports.
  pub fn fence(&self, run: RunId) -> Option<TeamFence> {
    self
      .fences
      .iter()
      .find(|(fenced, _)| *fenced == run)
      .map(|(_, fence)| *fence)
  }
}

/// The claims and fenced runs of the team project at `root`. Blocks on SQLite.
pub fn team_here(root: &Path) -> Result<TeamHere, StoreError> {
  let db = ProjectDb::open(root)?;
  let mut claims = Vec::new();
  let mut released = Vec::new();
  for operation in db.committed_team_operations()? {
    let Some(text) = operation.receipt.as_deref() else {
      continue;
    };
    // The receipt is the dispatcher's own JSON of a committed command; one this version cannot read names no claim it
    // took, and the server's reads still say who holds the subject.
    let Ok(receipt) = serde_json::from_str::<Receipt>(text) else {
      tracing::warn!(operation = %operation.id, "a kept team receipt cannot be read");
      continue;
    };
    match receipt.result {
      TeamCommandValue::Claim { claim, children, .. } => {
        claims.push(claim.id);
        claims.extend(children.iter().map(|child| child.id));
      }
      TeamCommandValue::Released { released_claim_id, .. } => released.push(released_claim_id),
      _ => {}
    }
  }
  let mut fences = Vec::new();
  for run in db.team_runs()? {
    match run.fence {
      Some(fence) => fences.push((run.run, fence)),
      None => claims.push(run.claim),
    }
  }
  claims.retain(|claim| !released.contains(claim));
  claims.sort_unstable();
  claims.dedup();
  Ok(TeamHere { claims, fences })
}
