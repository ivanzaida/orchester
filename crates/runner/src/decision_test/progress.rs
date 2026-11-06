use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::{
  connection::DecisionConnection,
  id::IdempotencyKey,
  resource::ModelId,
  run::review::{
    DecisionTestReport, DecisionTestStatus, Probability, ReviewSpend, ReviewVerdict, Reviewed, ThresholdFigures,
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::DecisionTestError;

pub(super) struct Progress {
  root: PathBuf,
  pub report: DecisionTestReport,
  latencies: Vec<u64>,
}

impl Progress {
  pub fn begin(
    root: &Path,
    db: &mut ProjectDb,
    id: IdempotencyKey,
    connection: &DecisionConnection,
    model: ModelId,
    records: &[orchester_store::RecordedDecisionReview],
  ) -> Result<Self, DecisionTestError> {
    let report = DecisionTestReport {
      id,
      connection: connection.id(),
      base_url: connection.base_url().clone(),
      key: connection.key(),
      model,
      started_at: Timestamp::now(),
      status: DecisionTestStatus::Running,
      requests: records.len() as u64,
      tested: 0,
      previously_asked: Some(records.iter().filter(|record| record.previously_asked).count() as u64),
      in_flight: 0,
      failed: 0,
      too_long: 0,
      median_ms: None,
      input_tokens: 0,
      unknown_usage: 0,
      thresholds: [0.5, 0.8, 0.9, 0.95]
        .into_iter()
        .map(|threshold| ThresholdFigures {
          threshold: Probability::new(threshold).expect("fixed valid threshold"),
          approves: 0,
          asks_person: 0,
          previously_asked_now_approved: 0,
        })
        .collect(),
      omitted_fields: vec![],
    };
    if !db.begin_decision_test(&report)? {
      return Err(DecisionTestError::AlreadySubmitted);
    }
    Ok(Self {
      root: root.to_path_buf(),
      report,
      latencies: vec![],
    })
  }

  pub fn omitted(&mut self, fields: Vec<String>) {
    for field in fields {
      if !self.report.omitted_fields.contains(&field) {
        self.report.omitted_fields.push(field);
      }
    }
  }

  pub fn too_long(&mut self) -> Result<(), DecisionTestError> {
    self.report.too_long += 1;
    self.save()
  }

  pub fn record(&mut self, reviewed: Reviewed, elapsed: Duration, asked: bool) -> Result<(), DecisionTestError> {
    self.report.in_flight -= 1;
    self.report.tested += 1;
    match reviewed.spend {
      ReviewSpend::Reported(usage) => self.report.input_tokens = self.report.input_tokens.saturating_add(usage.input),
      ReviewSpend::Unknown => self.report.unknown_usage += 1,
      ReviewSpend::Nothing => {}
    }
    let probability = match reviewed.verdict {
      Ok(ReviewVerdict::Decision(verdict)) => Some(verdict.approve),
      _ => {
        self.report.failed += 1;
        None
      }
    };
    for figures in &mut self.report.thresholds {
      if probability.is_some_and(|probability| probability >= figures.threshold) {
        figures.approves += 1;
        figures.previously_asked_now_approved += u64::from(asked);
      } else {
        figures.asks_person += 1;
      }
    }
    self
      .latencies
      .push(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
    self.latencies.sort_unstable();
    let middle = self.latencies.len() / 2;
    self.report.median_ms = Some(if self.latencies.len().is_multiple_of(2) {
      self.latencies[middle - 1] / 2
        + self.latencies[middle] / 2
        + (self.latencies[middle - 1] % 2 + self.latencies[middle] % 2) / 2
    } else {
      self.latencies[middle]
    });
    self.save()
  }

  pub fn finish(&mut self, status: DecisionTestStatus) -> Result<DecisionTestReport, DecisionTestError> {
    if status != DecisionTestStatus::Completed {
      self.report.unknown_usage += self.report.in_flight;
    }
    self.report.in_flight = 0;
    self.report.status = status;
    if let Err(error) = self.save() {
      self.report.status = DecisionTestStatus::Running;
      return Err(error);
    }
    Ok(self.report.clone())
  }

  pub(super) fn save(&self) -> Result<(), DecisionTestError> {
    ProjectDb::open(&self.root)?.save_decision_test(&self.report)?;
    Ok(())
  }
}

impl Drop for Progress {
  fn drop(&mut self) {
    if self.report.status == DecisionTestStatus::Running {
      self.report.unknown_usage += self.report.in_flight;
      self.report.in_flight = 0;
      self.report.status = DecisionTestStatus::Interrupted;
      if let Err(error) = self.save() {
        tracing::error!(%error, "cannot record interrupted decision test");
      }
    }
  }
}
