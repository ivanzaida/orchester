use orchester_core::{id::IdempotencyKey, resource::ModelId, run::review::DecisionTestReport};
use orchester_ipc::{DecisionTestTarget, Rejection};

use super::Scheduler;

impl Scheduler {
  pub async fn test_decision_connection(
    &self,
    id: IdempotencyKey,
    connection: DecisionTestTarget,
    model: ModelId,
    automation: bool,
  ) -> Result<DecisionTestReport, Rejection> {
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    crate::decision_test::test_in(
      &self.inner.project_root,
      self.inner.vault.as_ref(),
      id,
      connection,
      model,
      automation,
    )
    .await
    .map_err(|error| Rejection::Failed {
      detail: error.to_string(),
    })
  }
}
