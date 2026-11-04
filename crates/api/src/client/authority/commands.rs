//! The S4 command routes of one team project (`docs/cloud-api.md`, "Team authority P2 commands"): a team command sent
//! with its operation id, and the stored outcome of an operation read back after a lost response.

use orchester_core::id::OperationId;

use super::{Authority, AuthorityCallError};
use crate::v2::{CommandRequest, CommandResult, OperationRead, TeamCommand, TeamCommandValue};

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
}
