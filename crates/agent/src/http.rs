//! The HTTP side shared by the model clients (`docs/architecture/native-agent.md` §3.0): rustls with the
//! operating system's trust store, redirects never followed, a connect timeout and, instead of a total
//! timeout, an idle timeout on every wait for bytes. A proxy from `HTTPS_PROXY` (with `NO_PROXY`) is
//! used for remote endpoints; an endpoint on this computer is reached directly, since a proxy would see
//! its plain HTTP. No text that leaves here holds the API key.

use std::{future::Future, time::Duration};

use orchester_core::{
  command::{SecretVariable, redact_secrets},
  native::model::ModelError,
  resource::ModelId,
};
use reqwest::{
  Method, RequestBuilder, Response, StatusCode, Url,
  header::{HeaderValue, RETRY_AFTER},
  redirect::Policy,
};
use serde::Deserialize;

use crate::endpoint::{Endpoint, Timing};

/// The most of an error answer that is read.
const MAX_ERROR_BODY: usize = 64 * 1024;
/// The most of a JSON answer (a model list) that is read.
const MAX_JSON_BODY: usize = 16 * 1024 * 1024;

/// One endpoint's HTTP client.
#[derive(Clone, Debug)]
pub(crate) struct Http {
  client: reqwest::Client,
  endpoint: Endpoint,
  /// The key, for masking it in any text that leaves the client.
  redaction: Vec<SecretVariable>,
}

/// Why a request got no successful answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SendError {
  /// The endpoint answered with an error status.
  Answer(ErrorAnswer),
  /// No answer: the connection failed or went idle.
  Transport(ModelError),
}

/// An error answer: its status, the provider's error type and message, and `retry-after`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ErrorAnswer {
  pub status: u16,
  pub kind: Option<String>,
  pub message: String,
  pub retry_after: Option<Duration>,
}

#[derive(Deserialize)]
struct ErrorBody {
  error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
  #[serde(rename = "type")]
  kind: Option<String>,
  message: Option<String>,
}

impl Http {
  pub(crate) fn new(endpoint: Endpoint) -> Result<Self, ModelError> {
    // reqwest's rustls takes the process's crypto provider; ring is the one the workspace builds.
    // Installing fails only when a provider is installed already, which then serves as well.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut builder = reqwest::Client::builder()
      .redirect(Policy::none())
      .connect_timeout(endpoint.timing().connect);
    if endpoint.base_url().as_str().starts_with("http://") {
      builder = builder.no_proxy();
    }
    let redaction = vec![SecretVariable::new("API_KEY", endpoint.shared_key())];
    let client = builder.build().map_err(|error| transport(&error, &redaction))?;
    Ok(Self {
      client,
      endpoint,
      redaction,
    })
  }

  pub(crate) const fn endpoint(&self) -> &Endpoint {
    &self.endpoint
  }

  pub(crate) const fn timing(&self) -> Timing {
    self.endpoint.timing()
  }

  /// `{base}{path}`, `path` starting with `/`.
  pub(crate) fn url(&self, path: &str) -> Result<Url, ModelError> {
    Url::parse(&format!("{}{path}", self.endpoint.base_url().as_str()))
      .map_err(|error| ModelError::InvalidRequest(format!("the endpoint URL is invalid: {error}")))
  }

  /// `{base}{path}/{model}`, the model id encoded as one path segment.
  pub(crate) fn model_url(&self, path: &str, model: &ModelId) -> Result<Url, ModelError> {
    let mut url = self.url(path)?;
    url
      .path_segments_mut()
      .map_err(|()| ModelError::InvalidRequest("the endpoint URL cannot have a path".to_owned()))?
      .push(model.as_str());
    Ok(url)
  }

  pub(crate) fn request(&self, method: Method, url: Url) -> RequestBuilder {
    self.client.request(method, url)
  }

  /// The API key as a header value marked sensitive, so that it is never shown.
  pub(crate) fn key_header(&self) -> Result<HeaderValue, ModelError> {
    let mut value = HeaderValue::from_str(self.endpoint.key().expose()).map_err(|_| ModelError::KeyRejected)?;
    value.set_sensitive(true);
    Ok(value)
  }

  /// Sends `request` and waits for the answer's head within the idle timeout.
  pub(crate) async fn send(&self, request: RequestBuilder) -> Result<Response, SendError> {
    let response = self
      .idle(request.send())
      .await
      .map_err(SendError::Transport)?
      .map_err(|error| SendError::Transport(self.transport(&error)))?;
    if response.status().is_success() {
      return Ok(response);
    }
    Err(SendError::Answer(self.error_answer(response).await))
  }

  /// The next chunk of `response`'s body within the idle timeout; `None` at its end.
  pub(crate) async fn chunk(&self, response: &mut Response) -> Result<Option<Vec<u8>>, ModelError> {
    match self.idle(response.chunk()).await? {
      Ok(chunk) => Ok(chunk.map(|bytes| bytes.to_vec())),
      Err(error) => Err(self.transport(&error)),
    }
  }

  /// `response`'s whole body as JSON of type `T`.
  pub(crate) async fn json<T: for<'de> Deserialize<'de>>(&self, mut response: Response) -> Result<T, ModelError> {
    let body = self.read(&mut response, MAX_JSON_BODY).await?;
    serde_json::from_slice(&body)
      .map_err(|error| ModelError::Protocol(format!("the answer is not the expected JSON: {error}")))
  }

  /// Masks the API key in `text`.
  pub(crate) fn redact(&self, text: &str) -> String {
    redact_secrets(text, &self.redaction)
  }

  /// `error` with the API key masked in any text it holds.
  pub(crate) fn redact_error(&self, error: ModelError) -> ModelError {
    match error {
      ModelError::Transport(text) => ModelError::Transport(self.redact(&text)),
      ModelError::Protocol(text) => ModelError::Protocol(self.redact(&text)),
      ModelError::InvalidRequest(text) => ModelError::InvalidRequest(self.redact(&text)),
      other => other,
    }
  }

  pub(crate) fn transport(&self, error: &reqwest::Error) -> ModelError {
    transport(error, &self.redaction)
  }

  async fn idle<T>(&self, future: impl Future<Output = T>) -> Result<T, ModelError> {
    let idle = self.timing().idle;
    tokio::time::timeout(idle, future)
      .await
      .map_err(|_| ModelError::Transport(format!("the endpoint sent nothing for {} s", idle.as_secs_f32())))
  }

  async fn read(&self, response: &mut Response, limit: usize) -> Result<Vec<u8>, ModelError> {
    let mut body = Vec::new();
    while let Some(chunk) = self.chunk(response).await? {
      body.extend_from_slice(&chunk);
      if body.len() > limit {
        return Err(ModelError::Protocol(format!("the answer is longer than {limit} bytes")));
      }
    }
    Ok(body)
  }

  async fn error_answer(&self, mut response: Response) -> ErrorAnswer {
    let status = response.status();
    let retry_after = response
      .headers()
      .get(RETRY_AFTER)
      .and_then(|value| value.to_str().ok())
      .and_then(|value| value.trim().parse::<f64>().ok())
      .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
      .map(Duration::from_secs_f64);
    // An error answer whose body cannot be read in full still says what happened by its status.
    let body = self.read(&mut response, MAX_ERROR_BODY).await.unwrap_or_default();
    let (kind, message) = match serde_json::from_slice::<ErrorBody>(&body) {
      Ok(parsed) => (parsed.error.kind, parsed.error.message.unwrap_or_default()),
      Err(_) => (None, String::from_utf8_lossy(&body).trim().to_owned()),
    };
    ErrorAnswer {
      status: status.as_u16(),
      kind,
      message: self.redact(&message),
      retry_after,
    }
  }
}

impl ErrorAnswer {
  /// The error an answer with this status means (§3.1); `model` is the model the request named.
  pub(crate) fn into_error(self, model: Option<&ModelId>) -> ModelError {
    match StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR) {
      StatusCode::UNAUTHORIZED => ModelError::KeyRejected,
      StatusCode::PAYMENT_REQUIRED => ModelError::Billing,
      StatusCode::FORBIDDEN => ModelError::Forbidden,
      StatusCode::NOT_FOUND => model.map_or_else(
        || ModelError::Protocol(format!("the endpoint has no such path: {}", self.message)),
        |model| ModelError::ModelNotFound(model.clone()),
      ),
      StatusCode::PAYLOAD_TOO_LARGE => ModelError::TooLarge,
      StatusCode::TOO_MANY_REQUESTS => ModelError::RateLimited {
        retry_after: self.retry_after,
      },
      status if status.as_u16() == 529 => ModelError::Overloaded,
      status if status.is_server_error() => ModelError::Server(status.as_u16()),
      _ => ModelError::InvalidRequest(self.message),
    }
  }
}

/// A transport failure as text without the key; the URL it names holds none, but a proxy's might.
fn transport(error: &reqwest::Error, redaction: &[SecretVariable]) -> ModelError {
  let mut text = error.to_string();
  let mut source = std::error::Error::source(error);
  while let Some(inner) = source {
    text.push_str(": ");
    text.push_str(&inner.to_string());
    source = inner.source();
  }
  ModelError::Transport(redact_secrets(&text, redaction))
}
