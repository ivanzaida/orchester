//! Bounded, secret-safe decision review transport. The blocking worker owns this client, never the UI thread.
use std::time::Duration;

use orchester_core::{
  connection::{DecisionCheckResult, DecisionConnection},
  resource::ModelId,
  run::{
    TokenUsage,
    review::{
      Probability, RequestReviewer, ReviewError, ReviewFailure, ReviewInput, ReviewSpend, ReviewVerdict, Reviewed,
    },
  },
  secret::SecretValue,
};
use reqwest::{
  Client, Response, StatusCode,
  header::{AUTHORIZATION, HeaderValue},
};
use serde_json::Value;
use tokio::runtime::Handle;

use super::{parse_system_one_answer, system_one_request};

const TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BYTES: usize = 1024 * 1024;

/// A decision reviewer on its chosen server; no alternate reviewer or model is ever selected.
pub struct DecisionReviewer {
  runtime: Handle,
  connection: DecisionConnection,
  model: ModelId,
  threshold: Probability,
  key: Option<SecretValue>,
}

impl DecisionReviewer {
  pub fn new(
    runtime: Handle,
    connection: DecisionConnection,
    model: ModelId,
    threshold: Probability,
    key: Option<SecretValue>,
  ) -> Self {
    Self {
      runtime,
      connection,
      model,
      threshold,
      key,
    }
  }

  /// Recorded tests share the live transport, limits, parsing and secret handling.
  pub(crate) async fn review_body(&self, body: Value) -> Reviewed {
    let started = std::time::Instant::now();
    let mut reviewed = match tokio::time::timeout(TIMEOUT, self.request(body)).await {
      Ok(reviewed) => reviewed,
      Err(_) => Reviewed::new(Err(ReviewError::TimedOut), ReviewSpend::Unknown),
    };
    if let Ok(ReviewVerdict::Decision(decision)) = &mut reviewed.verdict {
      decision.elapsed_ms = Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    }
    reviewed
  }

  async fn request(&self, body: Value) -> Reviewed {
    // The runner may already have installed this process-wide provider.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = match Client::builder()
      .redirect(reqwest::redirect::Policy::none())
      .no_proxy()
      .timeout(TIMEOUT)
      .connect_timeout(TIMEOUT)
      .build()
    {
      Ok(client) => client,
      Err(_) => return failed(ReviewFailure::NothingAnswers, ReviewSpend::Nothing),
    };
    let authorization = match self.authorization() {
      Ok(header) => header,
      Err(error) => return failed(error, ReviewSpend::Nothing),
    };
    let mut catalog = client.get(format!("{}/models", self.connection.base_url()));
    if let Some(header) = &authorization {
      catalog = catalog.header(AUTHORIZATION, header.clone());
    }
    let models = match self.send(catalog).await {
      Ok(value) => value,
      Err(error) => return Reviewed::new(Err(error), ReviewSpend::Nothing),
    };
    if !listed(&models, &self.model) {
      return failed(ReviewFailure::ModelGone, ReviewSpend::Nothing);
    }
    let mut request = client
      .post(format!("{}/systemone", self.connection.base_url()))
      .json(&body);
    if let Some(header) = authorization {
      request = request.header(AUTHORIZATION, header);
    }
    let value = match self.send(request).await {
      Ok(value) => value,
      Err(error) => return Reviewed::new(Err(error), ReviewSpend::Unknown),
    };
    self.answer(value)
  }

  fn answer(&self, value: Value) -> Reviewed {
    let spend = value
      .get("usage")
      .and_then(|usage| usage.get("input_tokens"))
      .and_then(Value::as_u64)
      .filter(|input| *input <= i64::MAX as u64)
      .map_or(ReviewSpend::Unknown, |input| {
        ReviewSpend::Reported(TokenUsage {
          input,
          cached_input: 0,
          output: 0,
        })
      });
    // Keep the server's model and usage even when its six answers cannot be read.
    let reported_model = value
      .get("model")
      .and_then(Value::as_str)
      .and_then(|model| ModelId::new(model).ok());
    let verdict = if reported_model.is_some() {
      parse_system_one_answer(value, &self.model, self.threshold).map(ReviewVerdict::Decision)
    } else {
      Err(unreadable())
    };
    let mut reviewed = Reviewed::new(verdict, spend);
    reviewed.reported_model = reported_model;
    reviewed
  }

  fn authorization(&self) -> Result<Option<HeaderValue>, ReviewFailure> {
    let Some(key) = &self.key else {
      return if self.connection.key().is_some() {
        Err(ReviewFailure::KeyRefused)
      } else {
        Ok(None)
      };
    };
    let mut header =
      HeaderValue::from_str(&format!("Bearer {}", key.expose())).map_err(|_| ReviewFailure::KeyRefused)?;
    header.set_sensitive(true);
    Ok(Some(header))
  }

  async fn send(&self, request: reqwest::RequestBuilder) -> Result<Value, ReviewError> {
    let response = request.send().await.map_err(|error| transport_error(&error))?;
    match response.status() {
      StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => return Err(ReviewError::Decision(ReviewFailure::KeyRefused)),
      StatusCode::NOT_FOUND => return Err(ReviewError::Decision(ReviewFailure::NotSystemOneApi)),
      status if !status.is_success() => return Err(ReviewError::Decision(ReviewFailure::NothingAnswers)),
      _ => {}
    }
    let value = read(response).await?;
    // A server can echo credentials in JSON, including escaped text. No response string leaves this boundary unchecked.
    if self
      .key
      .as_ref()
      .is_some_and(|key| contains_secret(&value, key.expose()))
    {
      return Err(unreadable());
    }
    Ok(value)
  }
}

impl RequestReviewer for DecisionReviewer {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, input: &ReviewInput) -> Reviewed {
    if self.connection.key().is_some() && self.key.is_none() {
      return failed(ReviewFailure::KeyRefused, ReviewSpend::Nothing);
    }
    let failure = match self.connection.last_check().map(|check| &check.result) {
      Some(DecisionCheckResult::Connected) => None,
      Some(DecisionCheckResult::KeyRefused) => Some(ReviewFailure::KeyRefused),
      Some(DecisionCheckResult::NotSystemOneApi) => Some(ReviewFailure::NotSystemOneApi),
      Some(DecisionCheckResult::ModelGone { .. }) => Some(ReviewFailure::ModelGone),
      Some(DecisionCheckResult::NothingAnswers) | None => Some(ReviewFailure::NothingAnswers),
    };
    if let Some(failure) = failure {
      return failed(failure, ReviewSpend::Nothing);
    }
    if !self.connection.models().contains(&self.model) {
      return failed(ReviewFailure::ModelGone, ReviewSpend::Nothing);
    }
    self
      .runtime
      .block_on(self.review_body(system_one_request(input, &self.model)))
  }
}

fn failed(failure: ReviewFailure, spend: ReviewSpend) -> Reviewed {
  Reviewed::new(Err(ReviewError::Decision(failure)), spend)
}

fn transport_error(error: &reqwest::Error) -> ReviewError {
  if error.is_timeout() {
    ReviewError::TimedOut
  } else {
    ReviewError::Decision(ReviewFailure::NothingAnswers)
  }
}

fn unreadable() -> ReviewError {
  ReviewError::Unparseable("invalid System One response".to_owned())
}

async fn read(mut response: Response) -> Result<Value, ReviewError> {
  let mut bytes = Vec::new();
  while let Some(chunk) = response.chunk().await.map_err(|error| transport_error(&error))? {
    if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
      return Err(unreadable());
    }
    bytes.extend_from_slice(&chunk);
  }
  serde_json::from_slice(&bytes).map_err(|_| unreadable())
}

fn listed(value: &Value, model: &ModelId) -> bool {
  let models = value
    .get("models")
    .or_else(|| value.get("data"))
    .and_then(Value::as_array);
  models.is_some_and(|models| {
    models
      .iter()
      .any(|item| item.get("name").or_else(|| item.get("id")).and_then(Value::as_str) == Some(model.as_str()))
  })
}

fn contains_secret(value: &Value, key: &str) -> bool {
  match value {
    Value::String(text) => text.contains(key),
    Value::Array(items) => items.iter().any(|item| contains_secret(item, key)),
    Value::Object(items) => items
      .iter()
      .any(|(name, item)| name.contains(key) || contains_secret(item, key)),
    _ => false,
  }
}
