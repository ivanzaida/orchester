//! The model API boundary of the native agent (`docs/architecture/native-agent.md` §2.1, §3): one request
//! and one response per model turn. The clients that speak a provider's wire format over HTTP live outside
//! core (`orchester-agent`) and implement [`ModelClient`].

use std::{future::Future, time::Duration};

use thiserror::Error;

use super::{item::Item, tools::ToolSet};
use crate::{id::ConversationId, resource::ModelId, run::TokenUsage};

/// A model API reached over a provider's wire format.
pub trait ModelClient: Send + Sync {
  /// One model response. `events` receives text, reasoning and tool-call progress as it streams.
  fn respond(
    &self,
    request: &ModelRequest,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> impl Future<Output = Result<ModelResponse, ModelError>> + Send;

  /// The models the endpoint lists.
  fn models(&self) -> impl Future<Output = Result<Vec<RemoteModel>, ModelError>> + Send;
}

/// What one model turn is asked with. The system prompt and tools stay the same for a whole
/// conversation, and `items` only grow within a segment, so that providers can reuse their cache.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRequest {
  pub model: ModelId,
  /// The reasoning effort, as the model names it; `None` leaves the provider's default.
  pub effort: Option<String>,
  pub max_output: u32,
  pub system: String,
  pub tools: ToolSet,
  pub items: Vec<Item>,
  /// Groups the requests of one conversation for the provider's prompt cache.
  pub cache_key: ConversationId,
}

/// One model turn as the provider answered it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelResponse {
  pub items: Vec<Item>,
  pub stop: StopReason,
  pub usage: TokenUsage,
  /// Differs from the request's model after a provider fallback.
  pub served_by: ModelId,
}

/// Why the model stopped writing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
  EndTurn,
  ToolUse,
  /// The response reached the request's `max_output`.
  MaxOutput,
  /// The conversation filled the model's context window.
  ContextExceeded,
  /// The model declined to answer; `category` as the provider named it.
  Refusal {
    category: Option<String>,
  },
  /// The provider's content filter cut the response.
  Filtered,
}

/// Progress of a response while it streams.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
  Text(String),
  Reasoning(String),
  ToolCallStarted { name: String },
}

/// A model the endpoint lists, with the limits it reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteModel {
  pub id: ModelId,
  pub context_window: Option<u64>,
  pub max_output: Option<u32>,
  pub efforts: Vec<String>,
}

/// Why a model request failed after the client's own retries. Messages never hold the API key.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModelError {
  #[error("the API key was rejected")]
  KeyRejected,
  #[error("the account has a billing problem")]
  Billing,
  #[error("the API key may not use this endpoint or model")]
  Forbidden,
  #[error("the endpoint does not know model {}", .0.as_str())]
  ModelNotFound(ModelId),
  #[error("the request is too large for the endpoint")]
  TooLarge,
  #[error("the endpoint's rate limit was reached")]
  RateLimited { retry_after: Option<Duration> },
  #[error("the endpoint is overloaded")]
  Overloaded,
  #[error("the endpoint failed with status {0}")]
  Server(u16),
  #[error("the endpoint could not be reached: {0}")]
  Transport(String),
  #[error("the endpoint answered in an unexpected form: {0}")]
  Protocol(String),
  #[error("the endpoint refused the request: {0}")]
  InvalidRequest(String),
}
