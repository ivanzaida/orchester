//! The S4 command routes of one team project (`docs/cloud-api.md`, "Team authority P2 commands"): a team command sent
//! with its operation id, the stored outcome of an operation read back after a lost response, and the workspace
//! policy, which is changed on its own route under its revision rather than by a command.

use orchester_core::id::OperationId;
use reqwest::{Method, StatusCode};

use super::{Authority, AuthorityCallError};
use crate::{
  client::{ClientError, redirect, refused, retry_after},
  v2::{
    CommandRequest, CommandResult, OperationRead, TeamCommand, TeamCommandValue, WorkspacePolicy,
    WorkspacePolicyChanges, WorkspacePolicyError, WorkspacePolicyResponse,
  },
};

/// What `PATCH …/policy` made of a change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyChange {
  /// The policy as it is after the change, with the caller's capabilities.
  Changed(WorkspacePolicyResponse),
  /// `409 policy_revision_conflict`: the policy is no longer at the revision the change named; as it is now.
  Conflict(WorkspacePolicy),
}

impl Authority<'_> {
  /// `POST …/commands`: the receipt of the committed command. Sent again unchanged under the same operation id, it
  /// answers the original receipt; a refusal is [`AuthorityCallError::Refused`].
  pub async fn command(
    &self,
    request: &CommandRequest<TeamCommand>,
  ) -> Result<CommandResult<TeamCommandValue>, AuthorityCallError> {
    self.query("/commands", request).await
  }

  /// `GET …/operations/{operation}`: what the server made of the operation; `not_found` while it has no outcome of it.
  pub async fn operation(&self, operation: OperationId) -> Result<OperationRead<TeamCommandValue>, AuthorityCallError> {
    self.get(&format!("/operations/{operation}")).await
  }

  /// `GET …/policy`: the workspace's policy and what the caller may do with it.
  pub async fn policy(&self) -> Result<WorkspacePolicyResponse, AuthorityCallError> {
    self.get("/policy").await
  }

  /// `PATCH …/policy`: the automatic merge, made on the policy's revision; every accepted change moves the revision.
  /// The route's other refusals are the `{error, message}` form (`403` for a member), as [`ClientError::Api`].
  pub async fn change_policy(&self, changes: &WorkspacePolicyChanges) -> Result<PolicyChange, ClientError> {
    // Serializing the contract's own types fails only for a map with non-string keys, which it has not.
    let body = serde_json::to_vec(changes).map_err(|_| ClientError::Setup)?;
    let request = self
      .request(Method::PATCH, "/policy")?
      .header(reqwest::header::CONTENT_TYPE, "application/json")
      .body(body);
    let response = request.send().await.map_err(|error| ClientError::transport(&error))?;
    let status = response.status();
    if status.is_redirection() {
      return Err(redirect(&response));
    }
    let retry_after = retry_after(&response);
    let code = status.as_u16();
    let bytes = response.bytes().await.map_err(|error| ClientError::transport(&error))?;
    if status.is_success() {
      return serde_json::from_slice(&bytes)
        .map(PolicyChange::Changed)
        .map_err(|_| ClientError::NotOrchester { status: code });
    }
    if status == StatusCode::CONFLICT
      && let Ok(WorkspacePolicyError::PolicyRevisionConflict { current }) = serde_json::from_slice(&bytes)
    {
      return Ok(PolicyChange::Conflict(current));
    }
    Err(refused(&bytes, code, retry_after))
  }
}
