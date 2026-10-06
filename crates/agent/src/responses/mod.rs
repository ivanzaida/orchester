//! The OpenAI Responses client (`docs/architecture/native-agent.md` §3.2). It is stateless: the
//! provider keeps no conversation (`store: false`, no `previous_response_id`), every request sends the
//! whole conversation, and reasoning items come back encrypted, are kept in the conversation and are
//! sent again. Every request streams; failures are typed and retried as for Anthropic (§3.1); a response
//! cut off inside a tool call is asked again once with the model's whole output room; and a conversation
//! whose encrypted reasoning the endpoint cannot read (another organisation's key, say) is sent from then
//! on without the reasoning it had at that point.

mod models;
mod request;
mod stream;

use std::{collections::HashMap, time::Duration};

use orchester_core::{
  id::ConversationId,
  native::model::{ModelClient, ModelError, ModelRequest, ModelResponse, RemoteModel, StreamEvent},
  resource::ModelId,
};
use parking_lot::Mutex;
use reqwest::{
  Method, RequestBuilder,
  header::{AUTHORIZATION, CONTENT_TYPE},
};
use serde_json::Value;

use self::{
  models::{ModelList, known},
  request::Shape,
  stream::{Accumulator, Streamed},
};
use crate::{
  endpoint::Endpoint,
  http::{ErrorAnswer, Http, SendError},
};

/// A client of one OpenAI Responses endpoint.
#[derive(Debug)]
pub struct ResponsesClient {
  http: Http,
  /// Conversations sent without the reasoning items among their first items, and how many items that
  /// covers.
  stripped: Mutex<HashMap<ConversationId, usize>>,
}

/// Why one attempt failed.
#[derive(Clone, Debug)]
enum Failure {
  /// The error, with the `retry-after` the answer named, if any.
  Model(ModelError, Option<Duration>),
  /// The endpoint could not read the encrypted reasoning the request sent back; its message.
  ReasoningRefused(String),
}

impl From<ModelError> for Failure {
  fn from(error: ModelError) -> Self {
    Self::Model(error, None)
  }
}

impl ResponsesClient {
  pub fn new(endpoint: Endpoint) -> Result<Self, ModelError> {
    Ok(Self {
      http: Http::new(endpoint)?,
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
    let known = known(&request.model);
    let room = known.map_or(request.max_output, |known| known.max_output);
    let mut shape = Shape {
      max_output: request.max_output.min(room),
      // A model the table does not know reasons when the request names an effort for it.
      reasoning: known.is_some() || request.effort.is_some(),
      strip_before: self.stripped.lock().get(&request.cache_key).copied().unwrap_or(0),
    };
    let mut stripped_now = false;
    loop {
      match self.with_retries(request, shape, events).await {
        Ok(streamed) if streamed.cut_at_tool() && shape.max_output < room => {
          events(StreamEvent::Spent(streamed.usage));
          shape.max_output = room;
        }
        Ok(streamed) => {
          return Ok(ModelResponse {
            items: streamed.items,
            stop: streamed.stop,
            usage: streamed.usage,
            // The Responses API has no provider fallback: the requested model answered.
            served_by: request.model.clone(),
          });
        }
        Err(Failure::ReasoningRefused(_)) if !stripped_now => {
          stripped_now = true;
          shape.strip_before = request.items.len();
          self.stripped.lock().insert(request.cache_key, shape.strip_before);
          events(StreamEvent::ThinkingStripped);
        }
        Err(Failure::ReasoningRefused(message)) => return Err(ModelError::InvalidRequest(message)),
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
    let body = request::body(request, shape)?.to_string().into_bytes();
    let mut retry = 0;
    loop {
      match self.attempt(&body, &request.model, events).await {
        Err(Failure::Model(error, retry_after)) => {
          retry += 1;
          self.http.pause(retry, error, retry_after, events).await?;
        }
        other => return other,
      }
    }
  }

  async fn attempt(
    &self,
    body: &[u8],
    model: &ModelId,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<Streamed, Failure> {
    let url = self.http.url("/responses")?;
    let builder = self
      .authorized(self.http.request(Method::POST, url))?
      .header(CONTENT_TYPE, "application/json")
      .body(body.to_vec());
    let mut response = match self.http.send(builder).await {
      Ok(response) => response,
      Err(SendError::Transport(error)) => return Err(Failure::Model(error, None)),
      Err(SendError::Answer(answer)) => return Err(failure(answer, model)),
    };
    let mut response_so_far = Accumulator::new();
    let streamed = self
      .http
      .read_events(&mut response, |event| {
        response_so_far.event(event, events)?;
        Ok(response_so_far.ended())
      })
      .await;
    // Usage comes with the response's end only, so a stream that broke before it reports none.
    streamed.map_err(|error| Failure::Model(error, None))?;
    Ok(response_so_far.finish()?)
  }

  fn authorized(&self, builder: RequestBuilder) -> Result<RequestBuilder, ModelError> {
    Ok(builder.header(AUTHORIZATION, self.http.bearer_header()?))
  }

  async fn list_models(&self) -> Result<Vec<RemoteModel>, ModelError> {
    let builder = self.authorized(self.http.request(Method::GET, self.http.url("/models")?))?;
    let list: ModelList = match self.http.send(builder).await {
      Ok(response) => self.http.json(response).await?,
      Err(SendError::Answer(answer)) => return Err(answer_error(answer, None)),
      Err(SendError::Transport(error)) => return Err(error),
    };
    Ok(list.remote())
  }
}

impl ModelClient for ResponsesClient {
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

/// What an error answer to a Responses request means: encrypted reasoning the endpoint cannot read is
/// told apart from other refused requests.
fn failure(answer: ErrorAnswer, model: &ModelId) -> Failure {
  let code = answer.code.as_deref().unwrap_or_default();
  if answer.status == 400 && (code == "invalid_encrypted_content" || answer.message.contains("encrypted content")) {
    return Failure::ReasoningRefused(answer.message);
  }
  let retry_after = answer.retry_after;
  Failure::Model(answer_error(answer, Some(model)), retry_after)
}

/// The error an answer means (§3.1), with OpenAI's codes read first: a 429 for an exhausted quota is a
/// billing problem, which waiting does not fix, and a conversation over the context window is too
/// large, so that core compacts.
fn answer_error(answer: ErrorAnswer, model: Option<&ModelId>) -> ModelError {
  match answer.code.as_deref() {
    Some("insufficient_quota") => ModelError::Billing,
    Some("context_length_exceeded") => ModelError::TooLarge,
    _ => answer.into_error(model),
  }
}

/// The error an `error` event or a failed response reports, by its code: the response object's codes
/// as the API reference lists them, and the codes of error answers.
pub(super) fn stream_error(error: &Value) -> ModelError {
  let message = error["message"].as_str().unwrap_or_default().to_owned();
  match error["code"].as_str().unwrap_or_default() {
    "server_error" => ModelError::Server(500),
    "rate_limit_exceeded" => ModelError::RateLimited { retry_after: None },
    "insufficient_quota" => ModelError::Billing,
    "context_length_exceeded" => ModelError::TooLarge,
    "invalid_prompt" | "data_residency_mismatch" | "vector_store_timeout" => ModelError::InvalidRequest(message),
    image if image.contains("image") => ModelError::InvalidRequest(message),
    other => ModelError::Protocol(format!("the stream reported `{other}`: {message}")),
  }
}
