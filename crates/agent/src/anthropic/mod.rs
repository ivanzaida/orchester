//! The Anthropic Messages client (`docs/architecture/native-agent.md` §3.1). Rust has no official SDK, so
//! it speaks the wire format itself: every request streams, failures are typed and retried as the
//! design says, a response cut off inside a tool call is asked again once with the model's whole output
//! room, and a conversation whose thinking blocks the endpoint refuses as bound to another history is
//! sent from then on without the blocks it had at that point (the documented recovery).

mod models;
mod request;
mod stream;

use std::{collections::HashMap, time::Duration};

use orchester_core::{
  id::ConversationId,
  native::model::{ModelClient, ModelError, ModelRequest, ModelResponse, RemoteModel, StreamEvent},
  resource::ModelId,
  run::TokenUsage,
};
use parking_lot::Mutex;
use reqwest::{Method, RequestBuilder, Response, Url, header::CONTENT_TYPE};
use serde::Deserialize;
use serde_json::Value;

use self::{
  models::{ModelObject, ModelPage, ModelTraits},
  request::{Shape, VERSION},
  stream::{Accumulator, Streamed},
};
use crate::{
  endpoint::Endpoint,
  http::{ErrorAnswer, Http, SendError},
  retry,
  sse::SseParser,
};

/// Pages of `GET /v1/models` read at most; a listing that goes on longer is not a model list.
const MAX_MODEL_PAGES: usize = 50;

/// A client of one Anthropic Messages endpoint.
#[derive(Debug)]
pub struct AnthropicClient {
  http: Http,
  /// What each model used so far supports, from the Models API.
  traits: Mutex<HashMap<ModelId, ModelTraits>>,
  /// Conversations sent without the thinking blocks among their first items, and how many items that
  /// covers.
  stripped: Mutex<HashMap<ConversationId, usize>>,
}

/// Why one attempt failed.
#[derive(Clone, Debug)]
enum Failure {
  /// The error, with the `retry-after` the answer named, if any (§3.1: honoured for every retried
  /// status, not only 429).
  Model(ModelError, Option<Duration>),
  /// The endpoint refused a thinking block as bound to another history; its message.
  ThinkingBound(String),
}

impl From<ModelError> for Failure {
  fn from(error: ModelError) -> Self {
    Self::Model(error, None)
  }
}

impl AnthropicClient {
  pub fn new(endpoint: Endpoint) -> Result<Self, ModelError> {
    Ok(Self {
      http: Http::new(endpoint)?,
      traits: Mutex::new(HashMap::new()),
      stripped: Mutex::new(HashMap::new()),
    })
  }

  pub const fn endpoint(&self) -> &Endpoint {
    self.http.endpoint()
  }

  async fn respond_to(
    &self,
    request: &ModelRequest,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<ModelResponse, ModelError> {
    let traits = self.traits(&request.model, events).await?;
    let room = traits.max_output.unwrap_or(request.max_output);
    let mut shape = Shape {
      max_tokens: request.max_output.min(room),
      thinking: traits.adaptive_thinking,
      strip_before: self.stripped.lock().get(&request.cache_key).copied().unwrap_or(0),
    };
    let mut stripped_now = false;
    loop {
      match self.with_retries(request, shape, events).await {
        Ok(streamed) if streamed.cut_at_tool() && shape.max_tokens < room => {
          events(StreamEvent::Spent(streamed.usage));
          shape.max_tokens = room;
        }
        Ok(streamed) => {
          return Ok(ModelResponse {
            served_by: streamed.served_by(&request.model),
            items: streamed.items,
            stop: streamed.stop,
            usage: streamed.usage,
          });
        }
        Err(Failure::ThinkingBound(_)) if !stripped_now => {
          stripped_now = true;
          shape.strip_before = request.items.len();
          self.stripped.lock().insert(request.cache_key, shape.strip_before);
          events(StreamEvent::ThinkingStripped);
        }
        Err(Failure::ThinkingBound(message)) => return Err(ModelError::InvalidRequest(message)),
        Err(Failure::Model(error, _)) => return Err(error),
      }
    }
  }

  /// One response, with the retries §3.1 allows; each wait is reported before it starts.
  async fn with_retries(
    &self,
    request: &ModelRequest,
    shape: Shape,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<Streamed, Failure> {
    let endpoint = self.endpoint();
    let body = request::body(request, shape, endpoint)?.to_string().into_bytes();
    let betas = request::betas(shape, endpoint);
    let mut retry = 0;
    loop {
      match self.attempt(&body, &betas, &request.model, events).await {
        Err(Failure::Model(error, retry_after)) => {
          retry += 1;
          self.pause(retry, error, retry_after, events).await?;
        }
        other => return other,
      }
    }
  }

  /// Waits before retry `retry` after `error`, once the wait is reported; `Err(error)` when the request
  /// is not tried again (§3.1).
  async fn pause(
    &self,
    retry: u32,
    error: ModelError,
    retry_after: Option<Duration>,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<(), ModelError> {
    let Some(wait) = retry::wait(self.http.timing(), retry, &error, retry_after) else {
      return Err(error);
    };
    events(StreamEvent::Retrying {
      attempt: retry,
      wait,
      error: self.http.redact_error(error),
    });
    tokio::time::sleep(wait).await;
    Ok(())
  }

  async fn attempt(
    &self,
    body: &[u8],
    betas: &[&str],
    model: &ModelId,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<Streamed, Failure> {
    let url = self.http.url("/v1/messages")?;
    let builder = self
      .headers(self.http.request(Method::POST, url), betas)?
      .header(CONTENT_TYPE, "application/json")
      .body(body.to_vec());
    let mut response = match self.http.send(builder).await {
      Ok(response) => response,
      Err(SendError::Transport(error)) => return Err(Failure::Model(error, None)),
      Err(SendError::Answer(answer)) => return Err(failure(answer, model)),
    };
    let mut response_so_far = Accumulator::new();
    if let Err(error) = self.stream(&mut response, &mut response_so_far, events).await {
      let spent = response_so_far.usage();
      if spent != TokenUsage::default() {
        events(StreamEvent::Spent(spent));
      }
      return Err(Failure::Model(error, None));
    }
    Ok(response_so_far.finish()?)
  }

  /// Reads `response` into `into` up to `message_stop`.
  async fn stream(
    &self,
    response: &mut Response,
    into: &mut Accumulator,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<(), ModelError> {
    let mut parser = SseParser::new();
    loop {
      let Some(chunk) = self.http.chunk(response).await? else {
        return Err(ModelError::Transport(
          "the stream ended before the response was complete".to_owned(),
        ));
      };
      let parsed = parser
        .feed(&chunk)
        .map_err(|error| ModelError::Protocol(error.to_string()))?;
      for event in parsed {
        into.event(&event, events)?;
        if into.ended() {
          return Ok(());
        }
      }
    }
  }

  /// What `model` supports, asked of the Models API once per client, with the retries of a request.
  async fn traits(
    &self,
    model: &ModelId,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<ModelTraits, ModelError> {
    if let Some(traits) = self.traits.lock().get(model) {
      return Ok(*traits);
    }
    let url = self.http.model_url("/v1/models", model)?;
    let mut retry = 0;
    let object: ModelObject = loop {
      match self.get(url.clone(), Some(model)).await {
        Ok(object) => break object,
        Err(error) => {
          retry += 1;
          self.pause(retry, error, None, events).await?;
        }
      }
    };
    let traits = object.traits();
    self.traits.lock().insert(model.clone(), traits);
    Ok(traits)
  }

  async fn get<T: for<'de> Deserialize<'de>>(&self, url: Url, model: Option<&ModelId>) -> Result<T, ModelError> {
    let builder = self.headers(self.http.request(Method::GET, url), &[])?;
    match self.http.send(builder).await {
      Ok(response) => self.http.json(response).await,
      Err(SendError::Answer(answer)) => Err(answer.into_error(model)),
      Err(SendError::Transport(error)) => Err(error),
    }
  }

  fn headers(&self, builder: RequestBuilder, betas: &[&str]) -> Result<RequestBuilder, ModelError> {
    let mut builder = builder
      .header("x-api-key", self.http.key_header()?)
      .header("anthropic-version", VERSION);
    if !betas.is_empty() {
      builder = builder.header("anthropic-beta", betas.join(","));
    }
    Ok(builder)
  }

  async fn list_models(&self) -> Result<Vec<RemoteModel>, ModelError> {
    let mut models = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_MODEL_PAGES {
      let mut url = self.http.url("/v1/models")?;
      url.query_pairs_mut().append_pair("limit", "1000");
      if let Some(after) = &after {
        url.query_pairs_mut().append_pair("after_id", after);
      }
      let page: ModelPage = self.get(url, None).await?;
      for object in &page.data {
        if let Some(model) = object.remote() {
          self.traits.lock().insert(model.id.clone(), object.traits());
          models.push(model);
        }
      }
      match (page.has_more, page.last_id) {
        (true, Some(last)) => after = Some(last),
        _ => return Ok(models),
      }
    }
    Err(ModelError::Protocol(format!(
      "the model list goes on beyond {MAX_MODEL_PAGES} pages"
    )))
  }
}

impl ModelClient for AnthropicClient {
  async fn respond(
    &self,
    request: &ModelRequest,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<ModelResponse, ModelError> {
    let response = self.respond_to(request, events).await;
    response.map_err(|error| self.http.redact_error(error))
  }

  async fn models(&self) -> Result<Vec<RemoteModel>, ModelError> {
    let models = self.list_models().await;
    models.map_err(|error| self.http.redact_error(error))
  }
}

/// What an error answer to a Messages request means: a thinking block bound to another history and a
/// conversation too long for the context window are told apart from other refused requests (§3.1).
fn failure(answer: ErrorAnswer, model: &ModelId) -> Failure {
  if answer.status == 400 {
    let message = answer.message.to_lowercase();
    if message.contains("bound to a different conversation") {
      return Failure::ThinkingBound(answer.message);
    }
    if ["prompt is too long", "context window", "context limit"]
      .iter()
      .any(|wording| message.contains(wording))
    {
      return Failure::Model(ModelError::TooLarge, None);
    }
  }
  let retry_after = answer.retry_after;
  Failure::Model(answer.into_error(Some(model)), retry_after)
}

/// The error an `error` event in the stream reports, by its type.
pub(super) fn stream_error(error: &Value) -> ModelError {
  let message = error["message"].as_str().unwrap_or_default().to_owned();
  match error["type"].as_str().unwrap_or_default() {
    "overloaded_error" => ModelError::Overloaded,
    "rate_limit_error" => ModelError::RateLimited { retry_after: None },
    "api_error" => ModelError::Server(500),
    "authentication_error" => ModelError::KeyRejected,
    "billing_error" => ModelError::Billing,
    "permission_error" => ModelError::Forbidden,
    "request_too_large" => ModelError::TooLarge,
    "invalid_request_error" => ModelError::InvalidRequest(message),
    other => ModelError::Protocol(format!("the stream reported `{other}`: {message}")),
  }
}
