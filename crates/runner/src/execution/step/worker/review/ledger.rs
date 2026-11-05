//! What a review cost, recorded when it ends, from the blocking pool it ran on: whether the worker still
//! waits for it or not (the person answered first, the agent ended), a reviewer that was asked has spent
//! its tokens. The worker reports its end only after every ledger is gone (`Worker::work`), so the run
//! always hears of a review's tokens while its agent's invocation is still the step's. The record goes to the project's
//! store; known tokens go to the run, which counts them toward its budget, the step's and the project's daily budget
//! like an agent's turn; the plan limits its CLI reported go to the plan limits history and the desktop.

use std::{convert::Infallible, path::PathBuf};

use orchester_core::{
  connection::plan_limits::LimitReading,
  id::{InvocationId, RunId, StepExecutionId},
  run::review::{AutoReview, ReviewEnd, ReviewRecord},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use super::{super::WorkerEvent, setup::SpentOn};
use crate::notify::Notifier;

/// Where the cost of one review goes.
pub(super) struct ReviewLedger {
  pub(super) root: PathBuf,
  pub(super) run: RunId,
  pub(super) execution: StepExecutionId,
  pub(super) invocation: InvocationId,
  /// The request under review.
  pub(super) approval: String,
  pub(super) events: mpsc::UnboundedSender<WorkerEvent>,
  pub(super) notifier: Notifier,
  /// Keeps the worker from reporting its end until this review has recorded what it cost.
  pub(super) _held: mpsc::Sender<Infallible>,
}

impl ReviewLedger {
  pub(super) fn record_decision(
    &self,
    connection: &orchester_core::connection::DecisionConnection,
    model: &orchester_core::resource::ModelId,
    started_at: Timestamp,
    done: &AutoReview,
  ) {
    let Some(end) = ReviewEnd::of(&done.review) else {
      return;
    };
    let cost = done.cost.as_ref();
    let usage = cost.and_then(|cost| cost.usage);
    let record = orchester_core::run::review::DecisionReviewRecord {
      run: self.run,
      execution: self.execution,
      invocation: self.invocation,
      approval: self.approval.clone(),
      connection: connection.id(),
      provider: connection.name().as_str().to_owned(),
      model: cost.map_or_else(|| model.clone(), |cost| cost.model.clone()),
      started_at,
      ended_at: Timestamp::now(),
      input_tokens: usage.map(|usage| usage.input),
      end,
    };
    match ProjectDb::open(&self.root).and_then(|mut db| db.record_decision_reviewer_usage(&record)) {
      Ok(()) => {}
      Err(error) => tracing::warn!(%error, "cannot record decision reviewer usage"),
    }
    if let Some(usage) = usage
      && usage.input > 0
      && self
        .events
        .send(WorkerEvent::Usage {
          execution: self.execution,
          tokens: usage.counted_tokens(),
        })
        .is_err()
    {
      tracing::debug!("the run no longer counts its decision reviewer's usage");
    }
  }

  /// Records what the review by `provider` on `account` and `connection` that started at `started_at` cost, when it
  /// asked a model.
  pub(super) fn record(&self, (provider, account, connection): SpentOn, started_at: Timestamp, done: &AutoReview) {
    let (Some(cost), Some(end)) = (&done.cost, ReviewEnd::of(&done.review)) else {
      return;
    };
    let record = ReviewRecord {
      run: self.run,
      execution: self.execution,
      invocation: self.invocation,
      approval: self.approval.clone(),
      provider,
      account,
      connection: Some(connection),
      model: cost.model.clone(),
      started_at,
      ended_at: Timestamp::now(),
      usage: cost.usage,
      end,
    };
    let readings: Vec<LimitReading> = cost
      .limits
      .iter()
      .map(|limits| LimitReading {
        provider,
        account,
        at: record.ended_at,
        limits: limits.clone(),
      })
      .collect();
    // Stored before the run hears of the tokens, so the daily budget it reads counts them.
    self.store(&record, &readings);
    let tokens = record.counted_tokens();
    if tokens.get() > 0 {
      let usage = WorkerEvent::Usage {
        execution: self.execution,
        tokens,
      };
      if self.events.send(usage).is_err() {
        tracing::debug!("the run no longer counts its reviewer's usage");
      }
    }
    for reading in readings {
      self.notifier.send(Event::PlanLimits(reading));
    }
  }

  /// Stores the review's record and the plan limits it reported. A failure is logged: the review has
  /// ended and its verdict stands, only its record (and with it the daily budget's and the dashboard's
  /// share of it) is lost.
  fn store(&self, record: &ReviewRecord, readings: &[LimitReading]) {
    let mut db = match ProjectDb::open(&self.root) {
      Ok(db) => db,
      Err(error) => {
        tracing::warn!(%error, "cannot open the project to record what a review cost");
        return;
      }
    };
    if let Err(error) = db.record_reviewer_usage(record) {
      tracing::warn!(%error, "cannot record what a review cost");
    }
    for reading in readings {
      if let Err(error) = db.save_limit_snapshot(self.run, reading) {
        tracing::warn!(%error, "cannot store the plan limits the reviewer's CLI reported");
      }
    }
  }
}
