//! Requests and responses on the wire: the request's shape, a streamed turn read into items, the
//! append-only history with its cache reads, eager tool input, the output limit and refusals.

use orchester_agent::{AnthropicClient, WireQuirks};
use orchester_core::{
  connection::{ApiEndpoint, BaseUrl, EndpointPreset},
  id::{ConversationId, SecretId},
  native::{
    item::{Item, ToolCallId},
    model::{StopReason, StreamEvent},
    tools::{ToolInputError, parse_call},
  },
  run::TokenUsage,
};
use serde_json::{Value, json};

use crate::{
  KEY, client, endpoint,
  events::{self, MODEL},
  request, respond,
  server::{FakeServer, Part, Script},
  task,
};

const READ_INPUT: &str = r#"{"path":"src/main.rs","offset":null,"limit":null}"#;

fn tool_names(body: &Value) -> Vec<String> {
  body["tools"]
    .as_array()
    .expect("tools")
    .iter()
    .map(|tool| tool["name"].as_str().expect("a name").to_owned())
    .collect()
}

#[tokio::test]
async fn anthropic_request_has_the_designed_shape() {
  let script = Script::new()
    .expect(|request| {
      assert_eq!(request.header("x-api-key"), Some(KEY));
      assert_eq!(request.header("anthropic-version"), Some("2023-06-01"));
      assert_eq!(
        request.header("anthropic-beta"),
        Some("thinking-binding-controls-2026-08-01")
      );
      let body = request.json();
      assert_eq!(body["model"], json!(MODEL));
      assert_eq!(body["stream"], json!(true));
      assert_eq!(body["max_tokens"], json!(64_000));
      assert_eq!(body["system"][0]["cache_control"], json!({"type": "ephemeral"}));
      assert_eq!(body["cache_control"], json!({"type": "ephemeral"}));
      let names = tool_names(&body);
      let mut sorted = names.clone();
      sorted.sort();
      assert_eq!(names, sorted);
      for tool in body["tools"].as_array().expect("tools") {
        assert_eq!(tool["strict"], json!(true));
        assert_eq!(tool["eager_input_streaming"], json!(true));
        assert_eq!(tool["input_schema"]["additionalProperties"], json!(false));
      }
      assert_eq!(
        body["thinking"],
        json!({"type": "adaptive", "display": "summarized", "block_binding": {"prefix_mismatch_behavior": "error"}})
      );
      assert_eq!(body["output_config"], json!({"effort": "high"}));
      assert!(body.get("fallbacks").is_none());
      assert!(body.get("tool_choice").is_none());
      assert_eq!(
        body["messages"],
        json!([{"role": "user", "content": [{"type": "text", "text": "Fix the bug."}]}])
      );
    })
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Fix the bug.")]),
  )
  .await;
  let response = result.expect("a response");
  assert_eq!(
    response.items,
    vec![Item::Assistant {
      text: "Done.".to_owned()
    }]
  );
  assert_eq!(response.stop, StopReason::EndTurn);
  assert!(seen.contains(&StreamEvent::Text("Done.".to_owned())));
  server.finish();
}

#[tokio::test]
async fn anthropic_streamed_turn_becomes_items_with_usage() {
  let mut parts = vec![events::message_start(10, 20, &json!(30))];
  parts.extend(events::thinking(0, "Plan the read.", "sig-1"));
  parts.extend(events::text(1, "Reading."));
  parts.extend(events::tool_use(2, "toolu_1", "read_file", READ_INPUT, true));
  parts.extend([events::message_delta("tool_use", 7), events::message_stop()]);
  let server = FakeServer::start(Script::new().reply_sse(parts), events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  let response = result.expect("a response");
  assert_eq!(response.stop, StopReason::ToolUse);
  assert_eq!(
    response.usage,
    TokenUsage {
      input: 60,
      cached_input: 30,
      output: 7
    }
  );
  assert_eq!(response.served_by.as_str(), MODEL);
  let [
    Item::Reasoning { summary, opaque },
    Item::Assistant { text },
    Item::ToolCall { id, name, input },
  ] = response.items.as_slice()
  else {
    panic!("unexpected items: {:?}", response.items);
  };
  assert_eq!(summary.as_deref(), Some("Plan the read."));
  let block: Value = serde_json::from_str(opaque.json()).expect("JSON");
  assert_eq!(
    block,
    json!({"type": "thinking", "thinking": "Plan the read.", "signature": "sig-1"})
  );
  assert_eq!(text, "Reading.");
  assert_eq!(id, &ToolCallId::new("toolu_1").expect("an id"));
  assert_eq!(name, "read_file");
  assert_eq!(input, &serde_json::from_str::<Value>(READ_INPUT).expect("JSON"));
  assert!(seen.contains(&StreamEvent::Reasoning("Plan the read.".to_owned())));
  assert!(seen.contains(&StreamEvent::ToolCallStarted {
    name: "read_file".to_owned()
  }));
  server.finish();
}

#[tokio::test]
async fn anthropic_history_is_append_only_and_hits_the_cache() {
  let first = events::response(
    vec![
      events::thinking(0, "Look first.", "sig-a"),
      events::tool_use(1, "toolu_1", "read_file", READ_INPUT, true),
    ],
    "tool_use",
  );
  let mut second = vec![events::message_start(10, 0, &json!("echo"))];
  second.extend(events::text(0, "Done."));
  second.extend([events::message_delta("end_turn", 3), events::message_stop()]);
  let script = Script::new()
    .reply_sse(first)
    .expect(|request| {
      let messages = request.json()["messages"].clone();
      assert_eq!(
        messages[1]["content"][0],
        json!({"type": "thinking", "thinking": "Look first.", "signature": "sig-a"})
      );
      assert_eq!(messages[2]["content"][0]["type"], json!("tool_result"));
    })
    .reply_sse(second);
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let client = client(&server);
  let conversation = ConversationId::generate();
  let mut items = vec![task("Fix it.")];
  let (first, _) = respond(&client, &request(conversation, items.clone())).await;
  items.extend(first.expect("a first response").items);
  items.push(Item::tool_result(
    ToolCallId::new("toolu_1").expect("an id"),
    "1\tfn main() {}",
    false,
  ));
  let (second, _) = respond(&client, &request(conversation, items)).await;
  assert!(second.expect("a second response").usage.cached_input > 0);
  server.finish();
}

#[tokio::test]
async fn anthropic_invalid_tool_json_is_kept_for_an_error_result() {
  let raw = r#"{"path": "a.rs", "offset": nul"#;
  let parts = events::response(vec![events::tool_use(0, "toolu_9", "read_file", raw, true)], "tool_use");
  let server = FakeServer::start(Script::new().reply_sse(parts), events::catalog(), 10).await;
  let (result, _) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  let response = result.expect("a response");
  let [Item::ToolCall { name, input, .. }] = response.items.as_slice() else {
    panic!("unexpected items: {:?}", response.items);
  };
  assert_eq!(input, &json!({"INVALID_JSON": raw}));
  let error = parse_call(name, input).expect_err("not JSON");
  assert_eq!(error, ToolInputError::NotJson(raw.to_owned()));
  let result: Value = serde_json::from_str(&error.to_string()).expect("the error result is JSON");
  assert_eq!(result, json!({"INVALID_JSON": raw}));
  server.finish();
}

#[tokio::test]
async fn anthropic_max_tokens_inside_a_tool_call_asks_again_with_room() {
  let cut = events::response(
    vec![events::tool_use(0, "toolu_1", "read_file", READ_INPUT, false)],
    "max_tokens",
  );
  let whole = events::response(
    vec![events::tool_use(0, "toolu_1", "read_file", READ_INPUT, true)],
    "tool_use",
  );
  let script = Script::new()
    .expect(|request| assert_eq!(request.json()["max_tokens"], json!(64_000)))
    .reply_sse(cut)
    .expect(|request| assert_eq!(request.json()["max_tokens"], json!(128_000)))
    .reply_sse(whole);
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  let response = result.expect("a response");
  assert_eq!(response.stop, StopReason::ToolUse);
  assert!(
    seen
      .iter()
      .any(|event| matches!(event, StreamEvent::Spent(usage) if usage.output == 5))
  );
  server.finish();
}

#[tokio::test]
async fn anthropic_max_tokens_without_room_keeps_the_cut_input() {
  let cut = events::response(
    vec![events::tool_use(0, "toolu_1", "read_file", READ_INPUT, false)],
    "max_tokens",
  );
  let server = FakeServer::start(Script::new().reply_sse(cut), events::catalog(), 10).await;
  let mut asked = request(ConversationId::generate(), vec![task("Go.")]);
  asked.max_output = 128_000;
  let (result, _) = respond(&client(&server), &asked).await;
  let response = result.expect("a response");
  assert_eq!(response.stop, StopReason::MaxOutput);
  let [Item::ToolCall { input, .. }] = response.items.as_slice() else {
    panic!("unexpected items: {:?}", response.items);
  };
  assert_eq!(input, &json!({"INVALID_JSON": READ_INPUT}));
  server.finish();
}

#[tokio::test]
async fn anthropic_refusal_names_its_category() {
  let mut parts = vec![events::message_start(10, 0, &json!(0))];
  parts.extend(events::text(0, "I can't help with that."));
  parts.push(Part::Event(json!({"type": "message_delta",
    "delta": {"stop_reason": "refusal", "stop_details": {"type": "refusal", "category": "cyber"}},
    "usage": {"output_tokens": 4}})));
  parts.push(events::message_stop());
  let server = FakeServer::start(Script::new().reply_sse(parts), events::catalog(), 10).await;
  let (result, _) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  assert_eq!(
    result.expect("a response").stop,
    StopReason::Refusal {
      category: Some("cyber".to_owned())
    }
  );
  server.finish();
}

#[tokio::test]
async fn anthropic_conservative_endpoint_sends_the_plain_wire_format() {
  let script = Script::new()
    .expect(|request| {
      assert_eq!(request.header("anthropic-beta"), None);
      let body = request.json();
      assert_eq!(body["system"], json!("You work on one step of a task."));
      assert!(body.get("cache_control").is_none());
      assert!(body["tools"][0].get("eager_input_streaming").is_none());
      assert_eq!(body["thinking"], json!({"type": "adaptive", "display": "summarized"}));
    })
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let client = AnthropicClient::new(endpoint(&server, WireQuirks::CONSERVATIVE)).expect("a client");
  let (result, _) = respond(&client, &request(ConversationId::generate(), vec![task("Go.")])).await;
  result.expect("a response");
  server.finish();
}

#[tokio::test]
async fn anthropic_model_without_adaptive_thinking_runs_without_it() {
  let haiku = events::model(MODEL, false, 64_000, &[]);
  let script = Script::new()
    .expect(|request| {
      assert_eq!(
        request.header("anthropic-beta"),
        Some("server-side-fallback-2026-07-01")
      );
      let body = request.json();
      assert!(body.get("thinking").is_none());
      assert_eq!(body["fallbacks"], json!("default"));
    })
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, vec![haiku], 10).await;
  let with_fallback = endpoint(&server, WireQuirks::OFFICIAL).with_provider_fallback(true);
  let client = AnthropicClient::new(with_fallback).expect("a client");
  let (result, _) = respond(&client, &request(ConversationId::generate(), vec![task("Go.")])).await;
  result.expect("a response");
  server.finish();
}

#[test]
fn anthropic_quirks_follow_the_preset_and_base_url() {
  let api = |preset, url: &str| ApiEndpoint::new(preset, BaseUrl::new(url).expect("a URL"), SecretId::generate());
  assert_eq!(
    WireQuirks::anthropic(&api(EndpointPreset::Official, "https://api.anthropic.com")),
    WireQuirks::OFFICIAL
  );
  assert_eq!(
    WireQuirks::anthropic(&api(EndpointPreset::Official, "https://proxy.example.com")),
    WireQuirks::CONSERVATIVE
  );
  assert_eq!(
    WireQuirks::anthropic(&api(EndpointPreset::Custom, "https://api.anthropic.com")),
    WireQuirks::CONSERVATIVE
  );
}

#[tokio::test]
#[should_panic(expected = "the history was edited")]
async fn anthropic_fake_server_catches_an_edited_history() {
  let script = Script::new()
    .reply_sse(events::text_response("One."))
    .reply_sse(events::text_response("Two."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let client = client(&server);
  let conversation = ConversationId::generate();
  respond(&client, &request(conversation, vec![task("First.")]))
    .await
    .0
    .expect("a response");
  respond(&client, &request(conversation, vec![task("Edited.")]))
    .await
    .0
    .expect("a response");
  server.finish();
}

#[test]
fn anthropic_endpoints_on_this_computer_bypass_proxies() {
  use std::sync::Arc;

  use orchester_agent::Endpoint;
  use orchester_core::secret::SecretValue;

  let reached = |url: &str| {
    let key = Arc::new(SecretValue::new(KEY.to_owned()).expect("a key"));
    Endpoint::new(BaseUrl::new(url).expect("a URL"), key, WireQuirks::OFFICIAL).reached_directly()
  };
  for local in [
    "HTTP://LOCALHOST:8080",
    "http://127.0.0.1:9",
    "https://localhost",
    "https://[::1]:443",
    "https://127.0.0.1",
  ] {
    assert!(reached(local), "{local}");
  }
  assert!(!reached("https://api.anthropic.com"));
}
