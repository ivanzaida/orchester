//! Story points (ORC-49): every task may carry an effort size on the Fibonacci scale ([`StoryPoints`]), set by a
//! person or estimated by automation ([`EstimateSource`]), with automation's short reason. A person's value always
//! wins: automation never sets points over it ([`check_automation`]). Automation's estimate is made from the task's
//! brief and criteria ([`EstimateBasis`]) and is wanted again when they change ([`StoryEstimate::wanted`]).
//!
//! Story points are sizes of work relative to the project's other tasks, not money or tokens: they are no cost
//! estimate (`docs/architecture/native-agent.md`, decision 8). After a task is merged, its actual effort is kept
//! beside its points ([`calibration`]), so later estimates read how points tracked reality.

pub mod calibration;
mod prompt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::prompt::{
  ESTIMATE_INSTRUCTIONS, ESTIMATE_SCHEMA, EstimateAnswer, EstimateInput, estimate_prompt, parse_estimate,
  parse_estimate_text,
};
use super::TaskDetails;
use crate::{
  connection::AgentProvider,
  id::TaskId,
  resource::ModelId,
  run::{TokenUsage, review::ReviewFailure},
  time::Timestamp,
};

/// The values story points take.
const SCALE: [u8; 7] = [1, 2, 3, 5, 8, 13, 21];

/// A task's effort size: 1, 2, 3, 5, 8, 13 or 21.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct StoryPoints(u8);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PointsError {
  #[error("story points are 1, 2, 3, 5, 8, 13 or 21, not {0}")]
  OffScale(u32),
}

impl StoryPoints {
  pub fn new(value: u32) -> Result<Self, PointsError> {
    u8::try_from(value)
      .ok()
      .filter(|value| SCALE.contains(value))
      .map(Self)
      .ok_or(PointsError::OffScale(value))
  }

  pub const fn get(self) -> u32 {
    self.0 as u32
  }

  /// Every value of the scale, smallest first.
  pub fn all() -> impl Iterator<Item = Self> {
    SCALE.into_iter().map(Self)
  }
}

impl TryFrom<u32> for StoryPoints {
  type Error = PointsError;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    Self::new(value)
  }
}

impl From<StoryPoints> for u32 {
  fn from(points: StoryPoints) -> Self {
    points.get()
  }
}

/// Who set a task's points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstimateSource {
  /// A person in the window; never overwritten by automation.
  Person,
  /// Orchester's estimate, or automation through `create_task` / `update_task` (the Orchestrator chat).
  Automation,
}

/// What automation's estimate was made from: the task's brief and acceptance criteria, as one text. A change of
/// either wants a new estimate; a change of the title alone does not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstimateBasis(String);

impl EstimateBasis {
  pub fn of(details: &TaskDetails) -> Self {
    let criteria: Vec<&str> = details
      .acceptance_criteria
      .iter()
      .map(|criterion| criterion.trim())
      .filter(|criterion| !criterion.is_empty())
      .collect();
    Self(serde_json::json!([details.brief.trim(), criteria]).to_string())
  }

  /// A basis as it was stored.
  pub const fn restored(text: String) -> Self {
    Self(text)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A task's points, who set them and, for automation's, why and from what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryEstimate {
  points: StoryPoints,
  source: EstimateSource,
  reason: Option<String>,
  basis: Option<EstimateBasis>,
  at: Timestamp,
}

impl StoryEstimate {
  /// A person's points: no reason, made from nothing automation compares.
  pub const fn person(points: StoryPoints, at: Timestamp) -> Self {
    Self {
      points,
      source: EstimateSource::Person,
      reason: None,
      basis: None,
      at,
    }
  }

  /// Automation's points made from `basis`, with `reason` kept to one plain line.
  pub fn automation(points: StoryPoints, reason: Option<&str>, basis: EstimateBasis, at: Timestamp) -> Self {
    Self {
      points,
      source: EstimateSource::Automation,
      reason: reason
        .map(crate::run::review::one_plain_line)
        .filter(|reason| !reason.is_empty()),
      basis: Some(basis),
      at,
    }
  }

  /// An estimate as it was stored.
  pub const fn restored(
    (points, source): (StoryPoints, EstimateSource),
    reason: Option<String>,
    basis: Option<EstimateBasis>,
    at: Timestamp,
  ) -> Self {
    Self {
      points,
      source,
      reason,
      basis,
      at,
    }
  }

  pub const fn points(&self) -> StoryPoints {
    self.points
  }

  pub const fn source(&self) -> EstimateSource {
    self.source
  }

  /// Automation's reason; `None` for a person's points or a value automation gave without one.
  pub fn reason(&self) -> Option<&str> {
    self.reason.as_deref()
  }

  pub const fn basis(&self) -> Option<&EstimateBasis> {
    self.basis.as_ref()
  }

  pub const fn at(&self) -> Timestamp {
    self.at
  }

  /// Whether a task whose points are `current` wants automation's estimate made from `basis`: it has none, or
  /// automation's was made from another brief or other criteria. Never over a person's value.
  pub fn wanted(current: Option<&Self>, basis: &EstimateBasis) -> bool {
    match current {
      None => true,
      Some(estimate) => estimate.source == EstimateSource::Automation && estimate.basis.as_ref() != Some(basis),
    }
  }
}

/// Why automation's points were not set.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PointsRefused {
  #[error("a person set this task's story points; automation does not change them")]
  PersonsValue,
}

/// Automation may set points over `current` unless a person set them.
pub fn check_automation(current: Option<&StoryEstimate>) -> Result<(), PointsRefused> {
  match current.map(StoryEstimate::source) {
    Some(EstimateSource::Person) => Err(PointsRefused::PersonsValue),
    Some(EstimateSource::Automation) | None => Ok(()),
  }
}

/// How a request for automation's estimate of a task ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum EstimateOutcome {
  /// The estimator gave these points, now the task's.
  Estimated { points: StoryPoints },
  /// A person set the task's points: nothing was asked.
  PersonsValue,
  /// Automation's points were made from the brief and criteria as they are: nothing was asked.
  UpToDate,
  /// An estimate from the brief and criteria as they are was tried already and failed: it is tried again once they
  /// change.
  TriedBefore,
  /// The estimator gave no points, for this reason.
  Failed { failure: ReviewFailure },
}

/// One request to the estimator that asked a model, with what it spent: its own kind of usage, counted toward the
/// project's daily budget like a review, and named apart in `get_usage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstimateRecord {
  pub task: TaskId,
  /// What the estimate was asked from: one is not asked again from the same brief and criteria.
  pub basis: EstimateBasis,
  pub provider: AgentProvider,
  pub model: ModelId,
  pub started_at: Timestamp,
  pub ended_at: Timestamp,
  /// `None`: what it spent could not be read.
  pub usage: Option<TokenUsage>,
  /// `None`: it gave points; else why not.
  pub failure: Option<ReviewFailure>,
}
