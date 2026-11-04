//! What became of a team action, as a tool answers it: the definition or receipt the server accepted, or the refusal
//! a person gets for the same action. A member's definition edit is the server's `not_admin`, the existing
//! `not_admin` refusal of the project data; a revision conflict is `changed_meanwhile`; a refusal for now or a lost
//! answer says the change may still be sent again under the same intent, never that it was made.

use orchester_api::{
  ErrorCode,
  v2::{AuthorityError, AuthorityErrorCode},
};
use orchester_store::StoreError;

use crate::{
  actions::{Accepted, ActionError, Refusal, TeamOutcome},
  app_mcp::{error::ToolError, tools::task_write::write_error},
  messages::Message,
};

/// The accepted definition or receipt of `outcome`; a refusal otherwise.
pub(super) fn accepted(outcome: TeamOutcome) -> Result<Accepted, ToolError> {
  match outcome {
    TeamOutcome::Completed(accepted) => Ok(accepted),
    // The command committed; what this computer had still to do of it is no part of the tool's answer.
    TeamOutcome::AcceptedPending { receipt, .. } | TeamOutcome::LocalEffectFailed { receipt, .. } => {
      Ok(Accepted::Command(receipt))
    }
    TeamOutcome::Rejected { refusal, retryable, .. } => Err(refused(&refusal, retryable)),
    TeamOutcome::AcceptanceUnknown { .. } => Err(ToolError::refused(
      "team_acceptance_unknown",
      vec![Message::new("appMcp.errors.teamAcceptanceUnknown")],
    )),
  }
}

/// The server's refusal of a team action.
fn refused(refusal: &Refusal, retryable: bool) -> ToolError {
  if retryable {
    return ToolError::refused("team_unavailable", vec![Message::new("appMcp.errors.teamUnavailable")]);
  }
  match refusal {
    Refusal::Command(error) => authority_refused(error),
    Refusal::Route {
      code: ErrorCode::NotAdmin | ErrorCode::Forbidden,
      ..
    } => StoreError::NotAdmin.into(),
    Refusal::Route {
      code: ErrorCode::RevisionConflict,
      ..
    }
    | Refusal::PolicyRevisionConflict
    | Refusal::Superseded => changed(),
    Refusal::Route { code, .. } => coded(serde_json::to_value(code)),
  }
}

/// A team command's refusal: `forbidden` for a member is `not_admin`, as a definition route says it.
pub(super) fn authority_refused(error: &AuthorityError) -> ToolError {
  match error.code() {
    AuthorityErrorCode::Forbidden => StoreError::NotAdmin.into(),
    AuthorityErrorCode::RevisionConflict => changed(),
    code => coded(serde_json::to_value(code)),
  }
}

/// A refusal under the server's own code, which serializes to its snake_case name.
fn coded(code: serde_json::Result<serde_json::Value>) -> ToolError {
  let code = code
    .ok()
    .and_then(|code| code.as_str().map(str::to_owned))
    .unwrap_or_else(|| "team_refused".to_owned());
  ToolError::refused(
    code.clone(),
    vec![Message::new("appMcp.errors.teamRefused").with("code", code)],
  )
}

/// The subject changed on the server since it was read: nothing was changed.
fn changed() -> ToolError {
  ToolError::refused(
    "changed_meanwhile",
    vec![Message::new("tasks.conflict.changedMeanwhile")],
  )
}

/// The dispatcher's refusal before anything was sent, or its failure.
pub(super) fn action_refused(error: ActionError) -> ToolError {
  match error {
    ActionError::Store(error) => error.into(),
    ActionError::Write(error) => write_error(error),
    ActionError::DecisionRequired(decision) => ToolError::refused(
      "decision_required",
      vec![Message::new("appMcp.errors.teamDecisionRequired").with("decision", format!("{decision:?}"))],
    ),
    ActionError::NothingToChange => ToolError::refused(
      "nothing_to_change",
      vec![Message::new("appMcp.errors.teamNothingToChange")],
    ),
    ActionError::NoTeamSession | ActionError::SessionChanged | ActionError::OtherProject => {
      ToolError::refused("team_signed_out", vec![Message::new("appMcp.errors.teamSignedOut")])
    }
    error => {
      tracing::warn!(%error, "a tool's team action failed");
      ToolError::refused(
        "team_action_failed",
        vec![Message::new("appMcp.errors.teamActionFailed")],
      )
    }
  }
}
