//! Explicit Test actions, with durable progress that the connection page can poll while awaiting the reply.
use std::path::Path;

use orchester_core::{
  id::{DecisionConnectionId, IdempotencyKey},
  resource::ModelId,
};
use orchester_ipc::{Command, Reply};

use super::{RequestError, RunRequestError, Runners};
use crate::connections::decision::Writer;

impl Runners {
  /// A caller supplies its trusted authority. No startup, check or connection save invokes this action.
  pub async fn test_decision_connection(
    &self,
    root: &Path,
    test: IdempotencyKey,
    connection: DecisionConnectionId,
    model: ModelId,
    writer: Writer,
  ) -> Result<(), RunRequestError> {
    match self
      .ask(
        Some(root),
        Command::TestDecisionConnection {
          test,
          connection,
          model,
          automation: writer == Writer::Automation,
        },
      )
      .await?
    {
      Reply::DecisionTested { test: ended } if ended == test => Ok(()),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to TestDecisionConnection");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }
}
