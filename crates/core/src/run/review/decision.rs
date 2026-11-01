//! A decision model's verdict (`docs/design/decision-reviewer.md`, What it is asked, Threshold): no text, but the
//! probability that the request should be approved, the threshold it was judged against and five risk probabilities.
//! At or above the threshold the request runs; below it the person decides, told the highest risk at 0.5 or more.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The lowest risk probability a card names.
const NAMED_RISK_FROM: f64 = 0.5;

/// A probability, between 0 and 1.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Probability(f64);

// A probability is never NaN, so equality is total.
impl Eq for Probability {}

/// Why a number is not a probability.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[error("{0} is not a probability between 0 and 1")]
pub struct NotProbability(pub f64);

impl Probability {
  /// `value`, when it lies between 0 and 1.
  pub fn new(value: f64) -> Result<Self, NotProbability> {
    if (0.0..=1.0).contains(&value) {
      // `+ 0.0` makes a negative zero a plain one.
      Ok(Self(value + 0.0))
    } else {
      Err(NotProbability(value))
    }
  }

  pub const fn value(self) -> f64 {
    self.0
  }
}

impl TryFrom<f64> for Probability {
  type Error = NotProbability;

  fn try_from(value: f64) -> Result<Self, Self::Error> {
    Self::new(value)
  }
}

impl From<Probability> for f64 {
  fn from(probability: Probability) -> Self {
    probability.0
  }
}

/// One of the five risks a decision model is asked about, in the order it is asked and ties are broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
  WritesOutsideWorktree,
  ReachesNetwork,
  ReadsCredential,
  DiscardsWork,
  DoesNotServeTask,
}

impl Risk {
  pub const ALL: [Self; 5] = [
    Self::WritesOutsideWorktree,
    Self::ReachesNetwork,
    Self::ReadsCredential,
    Self::DiscardsWork,
    Self::DoesNotServeTask,
  ];
}

/// The probability of each risk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Risks {
  pub writes_outside_worktree: Probability,
  pub reaches_network: Probability,
  pub reads_credential: Probability,
  pub discards_work: Probability,
  pub does_not_serve_task: Probability,
}

impl Risks {
  pub const fn of(&self, risk: Risk) -> Probability {
    match risk {
      Risk::WritesOutsideWorktree => self.writes_outside_worktree,
      Risk::ReachesNetwork => self.reaches_network,
      Risk::ReadsCredential => self.reads_credential,
      Risk::DiscardsWork => self.discards_work,
      Risk::DoesNotServeTask => self.does_not_serve_task,
    }
  }

  /// The risk a card names: the highest at 0.5 or more, the first asked of equal ones; none below 0.5.
  pub fn named(&self) -> Option<(Risk, Probability)> {
    Risk::ALL
      .into_iter()
      .map(|risk| (risk, self.of(risk)))
      .filter(|(_, probability)| probability.value() >= NAMED_RISK_FROM)
      .fold(None, |highest, (risk, probability)| match highest {
        Some((_, top)) if top >= probability => highest,
        _ => Some((risk, probability)),
      })
  }
}

/// What a decision model answered about one request, as it is recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionVerdict {
  /// The model asked, as the reviewer chose it.
  pub model: String,
  /// The model's version, as its server reported it; `None` when it reported none.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub version: Option<String>,
  /// The probability that the request should be approved.
  pub approve: Probability,
  /// The reviewer's threshold when it judged.
  pub threshold: Probability,
  pub risks: Risks,
}

impl DecisionVerdict {
  /// Whether the request runs without the person: its probability is at or above the threshold.
  pub fn approves(&self) -> bool {
    self.approve >= self.threshold
  }
}
