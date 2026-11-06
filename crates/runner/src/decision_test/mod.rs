//! An explicit comparison only. It never decides a live request or runs as part of agent work.
mod input;
mod progress;

use std::{
  path::Path,
  sync::Arc,
  time::{Duration, Instant},
};

use orchester_core::{
  id::IdempotencyKey,
  resource::ModelId,
  run::review::{DecisionTestReport, DecisionTestStatus, Probability},
  secret::SecretVault,
  store::SecretStore,
};
use orchester_ipc::DecisionTestTarget;
use orchester_store::{ProjectDb, StoreError};
use progress::Progress;
use thiserror::Error;
use tokio::task::JoinSet;

use crate::decision_review::DecisionReviewer;

#[derive(Debug, Error)]
pub enum DecisionTestError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("person_only")]
  PersonOnly,
  #[error("a decision test is already running or this request has already been submitted")]
  AlreadySubmitted,
  #[error("the decision connection changed since this test was submitted")]
  ConnectionChanged,
  #[error("the decision connection's key is missing, archived or unavailable")]
  KeyUnavailable,
  #[error("the decision test worker stopped unexpectedly")]
  WorkerStopped,
}

/// Called only for an explicit workspace request. The authority comes from its caller, never model input.
pub async fn test_in(
  root: &Path,
  vault: &dyn SecretVault,
  id: IdempotencyKey,
  target: DecisionTestTarget,
  model: ModelId,
  automation: bool,
) -> Result<DecisionTestReport, DecisionTestError> {
  let (reviewer, records, mut progress) = {
    let mut db = ProjectDb::open(root)?;
    let connection = db
      .decision_connection(target.connection)?
      .ok_or(StoreError::DecisionConnectionNotFound(target.connection))?;
    if connection.base_url() != &target.base_url || connection.key() != target.key {
      return Err(DecisionTestError::ConnectionChanged);
    }
    if automation && !connection.base_url().is_on_this_computer() {
      return Err(DecisionTestError::PersonOnly);
    }
    let key = match connection.key() {
      Some(id) => {
        let secret = db
          .secret(id)?
          .filter(|secret| !secret.is_archived())
          .ok_or(DecisionTestError::KeyUnavailable)?;
        Some(
          vault
            .read(&secret.vault_key(db.project().id()))
            .map_err(|_| DecisionTestError::KeyUnavailable)?,
        )
      }
      None => None,
    };
    let records = db.recorded_decision_reviews()?;
    let progress = Progress::begin(root, &mut db, id, &connection, model.clone(), &records)?;
    let reviewer = Arc::new(DecisionReviewer::new(
      tokio::runtime::Handle::current(),
      connection,
      model,
      Probability::new(0.5).expect("a fixed valid threshold"),
      key,
    ));
    (reviewer, records, progress)
  };
  let result = execute(root, reviewer, records, &mut progress).await;
  match result {
    Ok(status) => progress.finish(status),
    Err(error) => Err(error),
  }
}

async fn execute(
  root: &Path,
  reviewer: Arc<DecisionReviewer>,
  records: Vec<orchester_store::RecordedDecisionReview>,
  progress: &mut Progress,
) -> Result<DecisionTestStatus, DecisionTestError> {
  let mut records = records.into_iter();
  let mut workers = JoinSet::new();
  let mut poll = tokio::time::interval(Duration::from_millis(25));
  loop {
    if ProjectDb::open(root)?.decision_test_stop_requested(progress.report.id)? {
      return Ok(DecisionTestStatus::Stopped);
    }
    let pending = workers.len();
    while workers.len() < 8 {
      let Some(record) = records.next() else {
        break;
      };
      let (body, omitted) = match input::request(&record, &progress.report.model, root) {
        Ok(request) => request,
        Err(_) => {
          progress.too_long()?;
          continue;
        }
      };
      progress.omitted(omitted);
      let reviewer = reviewer.clone();
      progress.report.in_flight += 1;
      workers.spawn(async move {
        let started = Instant::now();
        let reviewed = reviewer.review_body(body).await;
        (reviewed, started.elapsed(), record.previously_asked)
      });
    }
    if workers.len() != pending {
      progress.save()?;
    }
    if workers.is_empty() {
      return Ok(DecisionTestStatus::Completed);
    }
    tokio::select! {
      result = workers.join_next() => {
        let (reviewed, elapsed, asked) = result.expect("at least one worker").map_err(|_| DecisionTestError::WorkerStopped)?;
        progress.record(reviewed, elapsed, asked)?;
      }
      _ = poll.tick() => {}
    }
  }
}

/// Called before a newly started runner serves requests; comparisons are never resumed automatically.
pub(crate) fn recover(root: &Path) {
  if let Err(error) = ProjectDb::open(root).and_then(|mut db| db.interrupt_decision_tests()) {
    tracing::error!(%error, "cannot mark interrupted decision tests");
  }
}
