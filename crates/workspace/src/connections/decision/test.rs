//! Read models and stop requests for explicit recorded-request tests. They send no project content themselves.
use std::path::Path;

use orchester_core::{
  id::{DecisionConnectionId, IdempotencyKey},
  run::review::DecisionTestReport,
};
use orchester_store::{ProjectDb, StoreError};

/// The most recent test for this server and key. Changing either hides the old endpoint's results.
pub fn latest_in(root: &Path, id: DecisionConnectionId) -> Result<Option<DecisionTestReport>, StoreError> {
  let db = ProjectDb::open(root)?;
  let Some(connection) = db.decision_connection(id)? else {
    return Ok(None);
  };
  Ok(
    db.latest_decision_test(id)?
      .filter(|report| report.base_url == *connection.base_url() && report.key == connection.key()),
  )
}

/// Progress of the submission whose reply the page awaits, including partial results after Stop.
pub fn progress_in(root: &Path, id: IdempotencyKey) -> Result<Option<DecisionTestReport>, StoreError> {
  ProjectDb::open(root)?.decision_test(id)
}

pub fn stop_in(root: &Path, id: IdempotencyKey) -> Result<bool, StoreError> {
  ProjectDb::open(root)?.stop_decision_test(id)
}
