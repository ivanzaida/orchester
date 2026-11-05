//! Results of an explicitly requested comparison against a project's recorded reviews.
use serde::{Deserialize, Serialize};

use crate::{
  connection::DecisionServerUrl,
  id::{DecisionConnectionId, IdempotencyKey, SecretId},
  resource::ModelId,
  time::Timestamp,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionTestStatus {
  Running,
  Completed,
  Stopped,
  Interrupted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThresholdFigures {
  pub threshold: super::Probability,
  pub approves: u64,
  pub asks_person: u64,
  pub previously_asked_now_approved: u64,
}

/// Progress and results share one durable record. Unknown usage is counted separately, never treated as zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionTestReport {
  pub id: IdempotencyKey,
  pub connection: DecisionConnectionId,
  pub base_url: DecisionServerUrl,
  pub key: Option<SecretId>,
  pub model: ModelId,
  pub started_at: Timestamp,
  pub status: DecisionTestStatus,
  pub requests: u64,
  pub tested: u64,
  /// Requests in this test's source snapshot that the earlier reviewer left to the person.
  /// Older reports did not record this denominator.
  #[serde(default)]
  pub previously_asked: Option<u64>,
  pub in_flight: u64,
  pub failed: u64,
  pub too_long: u64,
  pub median_ms: Option<u64>,
  pub input_tokens: u64,
  pub unknown_usage: u64,
  pub thresholds: Vec<ThresholdFigures>,
  /// JSON paths omitted from reconstructed inputs because the original context was not stored.
  pub omitted_fields: Vec<String>,
}
