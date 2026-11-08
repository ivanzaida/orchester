//! `run_task`'s answer when the team server built no bundle for the start (`bundle_missing`): the reason it gives, in a
//! message per kind that says what to do ([`bundle_missing_message`]).

use orchester_api::v2::ErrorDetails;

use super::Team;
use crate::{
  app_mcp::{error::ToolError, services::blocking},
  messages::Message,
  team_work::{ClaimEnd, bundle_missing_message},
};

const CODE: &str = "team_bundle_missing";

/// The refusal for a start the server answered `bundle_missing` with `details`: nothing was started, and the claim
/// ended as `claim` says.
pub(super) async fn refused(team: &Team, details: &ErrorDetails, claim: Option<ClaimEnd>) -> ToolError {
  let reason = match details {
    ErrorDetails::BundleMissing { reason } => Some(reason.clone()),
    _ => None,
  };
  let root = team.open.repo_root.clone();
  match blocking(move || Ok(bundle_missing_message(&root, reason.as_ref())?)).await {
    Ok(message) => ToolError::refused(CODE, std::iter::once(message).chain(claim.map(claim_message)).collect()),
    Err(error) => error,
  }
}

fn claim_message(claim: ClaimEnd) -> Message {
  Message::new(match claim {
    ClaimEnd::Released => "appMcp.errors.teamClaimReleased",
    ClaimEnd::Kept => "appMcp.errors.teamClaimKept",
    ClaimEnd::Held => "appMcp.errors.teamClaimHeld",
  })
}
