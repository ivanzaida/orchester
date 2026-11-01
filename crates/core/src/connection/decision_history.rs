//! Recorded checks and benchmark figures of a decision model connection.

use serde::{Deserialize, Serialize};

use crate::{resource::ModelId, run::review::Probability, time::Timestamp};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionCheck {
  pub at: Timestamp,
  pub result: DecisionCheckResult,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionCheckResult {
  Connected,
  KeyRefused,
  NothingAnswers,
  NotSystemOneApi,
  ModelGone { model: ModelId },
}

/// The last test of one model, with one row for each threshold tested.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionTest {
  pub at: Timestamp,
  pub model: ModelId,
  pub figures: Vec<DecisionThresholdFigures>,
}

/// Counts of requests, elapsed milliseconds and token usage, rather than rounded display percentages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionThresholdFigures {
  pub threshold: Probability,
  pub approves_on_its_own: u64,
  pub comes_to_person: u64,
  pub approves_earlier_reviewer_asks: u64,
  pub requests_tested: u64,
  pub median_ms: u64,
  pub tokens: u64,
}
