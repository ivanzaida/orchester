//! The OpenAI Responses client against the fake model server (`docs/architecture/native-agent.md` §8,
//! layer 2): the cases of `anthropic_wire`, in the Responses wire format. No test reaches a real model
//! API.
//!
//! `cargo test -p orchester-agent --test responses_wire responses_`

mod events;
mod failures;
mod models;
#[path = "../fake_server/mod.rs"]
mod server;
mod stream;
mod wire;

use std::{sync::Arc, time::Duration};

use orchester_agent::{Endpoint, ResponsesClient, Timing, WireQuirks};
use orchester_core::{
  connection::BaseUrl,
  id::ConversationId,
  native::{
    item::{Item, UserOrigin},
    model::{ModelClient, ModelError, ModelRequest, ModelResponse, StreamEvent},
    tools::ToolSet,
  },
  permission::RepositoryAccess,
  resource::ModelId,
  secret::SecretValue,
};

use self::server::{FakeServer, Script, Wire};

/// The API key of every test; it must never show up in errors, events or `Debug` output.
pub const KEY: &str = "sk-proj-test-key-4b81d07ce95a";

/// Short waits, so that retries and the idle timeout take milliseconds.
pub const FAST: Timing = Timing {
  connect: Duration::from_secs(5),
  idle: Duration::from_millis(400),
  first_backoff: Duration::from_millis(5),
};

/// A Responses server playing `script`, with the default catalog.
pub async fn start(script: Script) -> FakeServer {
  FakeServer::start_wire(Wire::Responses, script, events::catalog(), 100).await
}

/// The server's endpoint, at `/v1` as OpenAI's base URL is.
pub fn endpoint(server: &FakeServer) -> Endpoint {
  let key = Arc::new(SecretValue::new(KEY.to_owned()).expect("a key"));
  let url = BaseUrl::new(&format!("{}/v1", server.base_url())).expect("a base URL");
  Endpoint::new(url, key, WireQuirks::CONSERVATIVE).with_timing(FAST)
}

pub fn client(server: &FakeServer) -> ResponsesClient {
  ResponsesClient::new(endpoint(server)).expect("a client")
}

pub fn request(conversation: ConversationId, items: Vec<Item>) -> ModelRequest {
  ModelRequest {
    model: ModelId::new(events::MODEL).expect("a model id"),
    effort: Some("high".to_owned()),
    max_output: 64_000,
    system: "You work on one step of a task.".to_owned(),
    tools: ToolSet::for_access(RepositoryAccess::Write),
    items,
    cache_key: conversation,
  }
}

pub fn task(text: &str) -> Item {
  Item::user(text, UserOrigin::Task)
}

/// One response with the events reported while it streamed.
pub async fn respond(
  client: &ResponsesClient,
  request: &ModelRequest,
) -> (Result<ModelResponse, ModelError>, Vec<StreamEvent>) {
  let mut seen = Vec::new();
  let result = client.respond(request, &mut |event| seen.push(event)).await;
  (result, seen)
}

/// One response to a single task item, from a fresh client of `server`.
pub async fn respond_once(server: &FakeServer) -> (Result<ModelResponse, ModelError>, Vec<StreamEvent>) {
  respond(&client(server), &request(ConversationId::generate(), vec![task("Go.")])).await
}
