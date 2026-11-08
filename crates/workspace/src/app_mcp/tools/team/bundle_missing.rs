//! `run_task`'s answer when the team server built no bundle for the start (`bundle_missing`): the reason it gives, in a
//! message per kind that says what to do ([`bundle_missing_message`]).

use orchester_api::v2::ErrorDetails;

use super::Team;
use crate::{
  app_mcp::{error::ToolError, services::blocking},
  team_work::bundle_missing_message,
};

const CODE: &str = "team_bundle_missing";

/// The refusal for a start the server answered `bundle_missing` with `details`: nothing was started.
pub(super) async fn refused(team: &Team, details: &ErrorDetails) -> ToolError {
  let reason = match details {
    ErrorDetails::BundleMissing { reason } => Some(reason.clone()),
    _ => None,
  };
  let root = team.open.repo_root.clone();
  match blocking(move || Ok(bundle_missing_message(&root, reason.as_ref())?)).await {
    Ok(message) => ToolError::refused(CODE, vec![message]),
    Err(error) => error,
  }
}
