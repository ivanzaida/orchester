//! The Anthropic Messages client against the fake model server (`docs/architecture/native-agent.md` §8,
//! layer 2). No test reaches a real model API.
//!
//! `cargo test -p orchester-agent --test anthropic_wire anthropic_`

mod events;
mod failures;
mod models;
mod review;
#[path = "../fake_server/mod.rs"]
mod server;
mod wire;

use std::{sync::Arc, time::Duration};

use orchester_agent::{AnthropicClient, Endpoint, Timing, WireQuirks};
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

use self::server::FakeServer;

/// The API key of every test; it must never show up in errors, events or `Debug` output.
pub const KEY: &str = "sk-ant-test-key-7f3a9c41e2d8";

/// Short waits, so that retries and the idle timeout take milliseconds.
pub const FAST: Timing = Timing {
  connect: Duration::from_secs(5),
  idle: Duration::from_millis(400),
  first_backoff: Duration::from_millis(5),
};

pub fn endpoint(server: &FakeServer, quirks: WireQuirks) -> Endpoint {
  let key = Arc::new(SecretValue::new(KEY.to_owned()).expect("a key"));
  Endpoint::new(BaseUrl::new(&server.base_url()).expect("a base URL"), key, quirks).with_timing(FAST)
}

pub fn client(server: &FakeServer) -> AnthropicClient {
  AnthropicClient::new(endpoint(server, WireQuirks::OFFICIAL)).expect("a client")
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
  client: &AnthropicClient,
  request: &ModelRequest,
) -> (Result<ModelResponse, ModelError>, Vec<StreamEvent>) {
  let mut seen = Vec::new();
  let result = client.respond(request, &mut |event| seen.push(event)).await;
  (result, seen)
}
