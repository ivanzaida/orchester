//! Decision reviewer settings and the data the reviewer row and hosted-choice question need.

use orchester_core::{
  connection::{Connection, DecisionCheckResult, DecisionConnection, DecisionThresholdFigures},
  id::DecisionConnectionId,
  resource::ModelId,
  run::review::{
    AutoModeSettings, DecisionReviewerConnection, Probability, ReviewerChoice, ReviewerConnectionId, ReviewerKind,
    ReviewerUnavailable, default_decision_threshold, resolve_reviewer,
  },
};
use orchester_store::{ProjectDb, StoreError};
use serde::Serialize;
use thiserror::Error;

/// No project content is sent while choosing; these are the parts sent when a request is reviewed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReviewerDestination {
  pub host: String,
  pub on_this_computer: bool,
  pub sends: [ReviewContent; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewContent {
  Task,
  Request,
  StepPermissionsAndContext,
  RecentAgentMessages,
}

impl ReviewerDestination {
  pub fn of(connection: &DecisionConnection) -> Self {
    Self {
      host: connection.base_url().host().to_owned(),
      on_this_computer: connection.is_on_this_computer(),
      sends: [
        ReviewContent::Task,
        ReviewContent::Request,
        ReviewContent::StepPermissionsAndContext,
        ReviewContent::RecentAgentMessages,
      ],
    }
  }
}

/// Retains the saved choice even when its connection or model is unavailable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReviewerRow {
  pub connection_id: Option<ReviewerConnectionId>,
  pub connection: Option<String>,
  pub kind: Option<ReviewerKind>,
  pub model: Option<ModelId>,
  pub effort: Option<String>,
  pub threshold: Option<Probability>,
  pub destination: Option<ReviewerDestination>,
  pub last_test: Option<DecisionThresholdFigures>,
  pub unavailable: Option<ReviewerUnavailable>,
  pub last_check: Option<DecisionCheckResult>,
  pub default: bool,
}

pub fn states(connections: &[DecisionConnection]) -> Vec<DecisionReviewerConnection<'_>> {
  connections
    .iter()
    .map(|connection| DecisionReviewerConnection {
      connection,
      connected: connection.last_check().is_some_and(|check| {
        matches!(
          check.result,
          DecisionCheckResult::Connected | DecisionCheckResult::ModelGone { .. }
        )
      }),
    })
    .collect()
}

pub fn row(settings: &AutoModeSettings, connections: &[Connection], decisions: &[DecisionConnection]) -> ReviewerRow {
  let resolved = resolve_reviewer(settings, connections, &states(decisions));
  // The .ok() reads below keep the successful fields; the resolution error is retained in unavailable.
  let choice = settings.reviewer.as_ref();
  let id = resolved
    .as_ref()
    .ok()
    .map(|r| r.connection.id())
    .or_else(|| choice.map(|c| c.connection));
  let decision = decisions
    .iter()
    .find(|c| Some(ReviewerConnectionId::Decision { decision: c.id() }) == id);
  let language = connections
    .iter()
    .find(|c| Some(ReviewerConnectionId::Language(c.id())) == id);
  let model = resolved
    .as_ref()
    .ok()
    .map(|r| r.model.clone())
    .or_else(|| choice.map(|c| c.model.clone()));
  let threshold = matches!(id, Some(ReviewerConnectionId::Decision { .. })).then(|| {
    choice
      .and_then(|c| c.threshold)
      .unwrap_or_else(default_decision_threshold)
  });
  ReviewerRow {
    connection_id: id,
    connection: decision
      .map(|c| c.name())
      .or_else(|| language.map(|c| c.name()))
      .map(|name| name.as_str().to_owned()),
    kind: if matches!(id, Some(ReviewerConnectionId::Decision { .. })) {
      Some(ReviewerKind::Decision)
    } else {
      resolved.as_ref().ok().map(|r| r.kind)
    },
    model: model.clone(),
    effort: if threshold.is_some() {
      None
    } else {
      choice.and_then(|c| c.effort.clone())
    },
    threshold,
    destination: decision.map(ReviewerDestination::of),
    last_test: decision
      .and_then(|c| c.last_test())
      .filter(|test| Some(&test.model) == model.as_ref())
      .and_then(|test| test.figures.iter().find(|figure| Some(figure.threshold) == threshold))
      .cloned(),
    unavailable: resolved.err(),
    last_check: decision.and_then(|c| c.last_check()).map(|check| check.result.clone()),
    default: choice.is_none(),
  }
}

pub fn read(db: &ProjectDb) -> Result<ReviewerRow, StoreError> {
  use orchester_core::store::ConnectionStore;
  Ok(row(
    &db.auto_mode_settings()?,
    &db.connections()?,
    &db.decision_connections()?,
  ))
}

#[derive(Debug, Error)]
pub enum DecisionReviewerError {
  #[error("the threshold must be between 0.01 and 0.99 with at most two decimal places")]
  Threshold,
  #[error("the decision model is not listed by this connection")]
  Model,
}

/// Decimal parsing rejects excess precision rather than silently rounding a person's choice.
pub fn threshold(value: &str) -> Result<Probability, DecisionReviewerError> {
  let value = value.trim();
  let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
  if whole != "0" || fraction.is_empty() || fraction.len() > 2 || !fraction.bytes().all(|c| c.is_ascii_digit()) {
    return Err(DecisionReviewerError::Threshold);
  }
  let parsed = value.parse::<f64>().map_err(|_| DecisionReviewerError::Threshold)?;
  if !(0.01..=0.99).contains(&parsed) {
    return Err(DecisionReviewerError::Threshold);
  }
  Probability::new(parsed).map_err(|_| DecisionReviewerError::Threshold)
}

pub fn choose(
  connection: &DecisionConnection,
  model: ModelId,
  threshold: &str,
) -> Result<ReviewerChoice, DecisionReviewerError> {
  if !connection.models().contains(&model) {
    return Err(DecisionReviewerError::Model);
  }
  Ok(ReviewerChoice::decision(
    connection.id(),
    model,
    Some(self::threshold(threshold)?),
  ))
}

/// Threshold edits on the same connection and model never choose a new destination.
pub fn choosing_hosted(
  base: &AutoModeSettings,
  asked: &AutoModeSettings,
  decisions: &[DecisionConnection],
) -> Option<ReviewerDestination> {
  let next = asked.reviewer.as_ref()?;
  let ReviewerConnectionId::Decision { decision } = next.connection else {
    return None;
  };
  if base
    .reviewer
    .as_ref()
    .is_some_and(|old| old.connection == next.connection && old.model == next.model)
  {
    return None;
  }
  decisions
    .iter()
    .find(|c| c.id() == decision && !c.is_on_this_computer())
    .map(ReviewerDestination::of)
}

pub fn connection_id(choice: &ReviewerChoice) -> Option<DecisionConnectionId> {
  match choice.connection {
    ReviewerConnectionId::Decision { decision } => Some(decision),
    ReviewerConnectionId::Language(_) => None,
  }
}
