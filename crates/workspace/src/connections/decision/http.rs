//! The model-list transport boundary. Credentials and response bodies never escape this module.
//! TypeSafe: https://docs.typesafe.ai/api/list-models (models[].name).
//! Laya: https://laya.studio/v1/openapi marks /v1/models public; its public endpoint
//! https://api.laya.studio/v1/models (2026-11-01) returns object=list, data[].id with checkpoint metadata.
//! A generic OpenAI model list alone is not evidence of a System One API.

use std::time::Duration;

use orchester_core::{
  connection::{DecisionCheckResult, DecisionConnection},
  id::ProjectId,
  resource::ModelId,
  secret::{SecretValue, SecretVault, VaultKey},
};
use reqwest::{
  Client, StatusCode,
  header::{AUTHORIZATION, HeaderValue},
};
use serde::Deserialize;

use super::DecisionError;

const MAX_MODEL_LIST_BYTES: usize = 1024 * 1024;

pub(super) async fn check(
  project: ProjectId,
  vault: &dyn SecretVault,
  connection: &DecisionConnection,
) -> Result<(DecisionCheckResult, Option<Vec<ModelId>>), DecisionError> {
  let key = match connection.key() {
    Some(id) => match vault.read(&VaultKey::new(project, id)) {
      Ok(key) => Some(key),
      // Vault diagnostics may contain platform details; neither they nor values are returned or logged.
      Err(_) => return Ok((DecisionCheckResult::KeyRefused, None)),
    },
    None => None,
  };
  check_with_key(connection.base_url(), key.as_ref()).await
}

pub(super) async fn check_with_key(
  url: &orchester_core::connection::DecisionServerUrl,
  key: Option<&SecretValue>,
) -> Result<(DecisionCheckResult, Option<Vec<ModelId>>), DecisionError> {
  let client = client()?;
  let mut request = client.get(format!("{}/models", url.as_str()));
  if let Some(key) = key {
    let Ok(mut authorization) = HeaderValue::from_str(&format!("Bearer {}", key.expose())) else {
      return Ok((DecisionCheckResult::KeyRefused, None));
    };
    authorization.set_sensitive(true);
    request = request.header(AUTHORIZATION, authorization);
  }
  let response = match request.send().await {
    Ok(response) => response,
    // reqwest errors can carry a URL or headers. Keep only the classified state.
    Err(_) => return Ok((DecisionCheckResult::NothingAnswers, None)),
  };
  match response.status() {
    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Ok((DecisionCheckResult::KeyRefused, None)),
    status if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS => {
      Ok((DecisionCheckResult::NothingAnswers, None))
    }
    status if status.is_success() => read_models(response, key).await,
    _ => Ok((DecisionCheckResult::NotSystemOneApi, None)),
  }
}

fn client() -> Result<Client, DecisionError> {
  // Another client may already have installed the same process-wide provider.
  let _ = rustls::crypto::ring::default_provider().install_default();
  Client::builder()
    .redirect(reqwest::redirect::Policy::none())
    .no_proxy()
    .connect_timeout(Duration::from_secs(5))
    .timeout(Duration::from_secs(10))
    .build()
    .map_err(|_| DecisionError::ClientUnavailable)
}

async fn read_models(
  mut response: reqwest::Response,
  key: Option<&SecretValue>,
) -> Result<(DecisionCheckResult, Option<Vec<ModelId>>), DecisionError> {
  let mut bytes = Vec::new();
  loop {
    match response.chunk().await {
      Ok(Some(chunk)) if bytes.len().saturating_add(chunk.len()) <= MAX_MODEL_LIST_BYTES => {
        bytes.extend_from_slice(&chunk);
      }
      Ok(None) => break,
      Ok(Some(_)) => return Ok((DecisionCheckResult::NotSystemOneApi, None)),
      Err(_) => return Ok((DecisionCheckResult::NothingAnswers, None)),
    }
  }
  // A server may echo its Authorization header in an otherwise valid JSON body. Never persist that echo
  // as a model id. Only parsed, validated ids leave this transport boundary.
  if key.is_some_and(|key| {
    bytes
      .windows(key.expose().len())
      .any(|part| part == key.expose().as_bytes())
  }) {
    return Ok((DecisionCheckResult::NotSystemOneApi, None));
  }
  match parse_models(&bytes) {
    Some(models) if !key.is_some_and(|key| models.iter().any(|model| model.as_str().contains(key.expose()))) => {
      Ok((DecisionCheckResult::Connected, Some(models)))
    }
    _ => Ok((DecisionCheckResult::NotSystemOneApi, None)),
  }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ModelList {
  TypeSafe { models: Vec<TypeSafeModel> },
  Laya { object: String, data: Vec<LayaModel> },
}

#[derive(Deserialize)]
struct TypeSafeModel {
  name: ModelId,
  // Required by TypeSafe's schema, but neither arbitrary descriptions nor dates enter read models.
  #[serde(rename = "description")]
  _description: String,
  #[serde(rename = "release_date")]
  _release_date: String,
}

#[derive(Deserialize)]
struct LayaModel {
  id: ModelId,
  repo: String,
  #[serde(rename = "backbone")]
  _backbone: String,
  #[serde(rename = "context_tokens")]
  _context_tokens: u64,
  #[serde(rename = "languages")]
  _languages: String,
}

fn parse_models(bytes: &[u8]) -> Option<Vec<ModelId>> {
  let value: serde_json::Value = match serde_json::from_slice(bytes) {
    Ok(value) => value,
    Err(_) => return None,
  };
  if value.get("models").is_some() && value.get("data").is_some() {
    return None;
  }
  match serde_json::from_value::<ModelList>(value) {
    Ok(ModelList::TypeSafe { models }) => Some(models.into_iter().map(|model| model.name).collect()),
    Ok(ModelList::Laya { object, data })
      if object == "list"
        && !data.is_empty()
        && data
          .iter()
          .all(|model| model.repo.starts_with("convaiinnovations/laya")) =>
    {
      Some(data.into_iter().map(|model| model.id).collect())
    }
    // Unsupported, malformed and ambiguous catalogs fail closed; no inference request is used to identify them.
    _ => None,
  }
}
