//! Failures: status codes mapped to typed errors, retries with backoff and `retry-after`, broken and
//! idle streams, the preserved-thinking recovery, and the API key kept out of every error.

use std::time::Duration;

use orchester_agent::{AnthropicClient, WireQuirks};
use orchester_core::{
  id::ConversationId,
  native::{
    item::{Item, ToolCallId},
    model::{ModelError, StreamEvent},
  },
  resource::ModelId,
};
use serde_json::json;

use crate::{
  KEY, client, endpoint,
  events::{self, MODEL},
  request, respond,
  server::{FakeServer, Part, Script},
  task,
};

const BOUND: &str = "messages.1.content.0: Invalid `signature` in `thinking` block. The block is bound to a \
                     different conversation. Remove the block.";

fn retries(seen: &[StreamEvent]) -> Vec<(u32, Duration, ModelError)> {
  seen
    .iter()
    .filter_map(|event| match event {
      StreamEvent::Retrying { attempt, wait, error } => Some((*attempt, *wait, error.clone())),
      _ => None,
    })
    .collect()
}

async fn one_error(script: Script) -> (ModelError, usize) {
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, _) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  let error = result.expect_err("an error");
  let count = server.messages_requests().len();
  server.finish();
  (error, count)
}

#[tokio::test]
async fn anthropic_statuses_map_to_typed_errors() {
  let model = ModelId::new(MODEL).expect("a model id");
  let cases = [
    (401, "authentication_error", "bad key", ModelError::KeyRejected),
    (402, "billing_error", "no credit", ModelError::Billing),
    (403, "permission_error", "not allowed", ModelError::Forbidden),
    (
      404,
      "not_found_error",
      "no such model",
      ModelError::ModelNotFound(model),
    ),
    (413, "request_too_large", "too large", ModelError::TooLarge),
    (
      400,
      "invalid_request_error",
      "messages: bad field",
      ModelError::InvalidRequest("messages: bad field".to_owned()),
    ),
    (
      400,
      "invalid_request_error",
      "prompt is too long: 1200000 tokens > 1000000 maximum",
      ModelError::TooLarge,
    ),
    (
      400,
      "invalid_request_error",
      "input length and `max_tokens` exceed context limit: 990000 + 64000 > 1000000",
      ModelError::TooLarge,
    ),
  ];
  for (status, kind, message, expected) in cases {
    let (error, requests) = one_error(Script::new().reply_error(status, kind, message)).await;
    assert_eq!(error, expected, "status {status}");
    assert_eq!(requests, 1, "status {status} is not retried");
  }
}

#[tokio::test]
async fn anthropic_rate_limit_waits_as_retry_after_says() {
  let body = json!({"type": "error", "error": {"type": "rate_limit_error", "message": "slow down"}}).to_string();
  let script = Script::new()
    .reply_status("/v1/messages", 429, &[("retry-after", "0")], &body)
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  result.expect("a response after the retry");
  assert_eq!(
    retries(&seen),
    vec![(
      1,
      Duration::ZERO,
      ModelError::RateLimited {
        retry_after: Some(Duration::ZERO)
      }
    )]
  );
  server.finish();
}

#[tokio::test]
async fn anthropic_server_errors_are_retried_with_backoff() {
  let script = Script::new()
    .reply_error(500, "api_error", "internal")
    .reply_error(529, "overloaded_error", "overloaded")
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  result.expect("a response after two retries");
  let waits = retries(&seen);
  assert_eq!(waits.len(), 2);
  assert_eq!((waits[0].0, &waits[0].2), (1, &ModelError::Server(500)));
  assert_eq!((waits[1].0, &waits[1].2), (2, &ModelError::Overloaded));
  assert!(
    waits[1].1 >= Duration::from_millis(10),
    "the second wait doubles: {:?}",
    waits[1].1
  );
  server.finish();
}

#[tokio::test]
async fn anthropic_retries_end_after_five() {
  let mut script = Script::new();
  for _ in 0..6 {
    script = script.reply_error(529, "overloaded_error", "overloaded");
  }
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  assert_eq!(result.expect_err("overloaded"), ModelError::Overloaded);
  assert_eq!(retries(&seen).len(), 5);
  assert_eq!(server.messages_requests().len(), 6);
  server.finish();
}

#[tokio::test]
async fn anthropic_broken_stream_is_dropped_and_retried() {
  let mut broken = vec![events::message_start(10, 0, &json!(0))];
  broken.extend(events::text(0, "Half").into_iter().take(2));
  broken.push(Part::Drop);
  let script = Script::new()
    .reply_sse(broken)
    .reply_sse(events::text_response("Whole."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  let response = result.expect("a response after the retry");
  assert_eq!(
    response.items,
    vec![Item::Assistant {
      text: "Whole.".to_owned()
    }]
  );
  let waits = retries(&seen);
  assert!(
    matches!(waits.as_slice(), [(1, _, ModelError::Transport(_))]),
    "{waits:?}"
  );
  assert!(
    seen
      .iter()
      .any(|event| matches!(event, StreamEvent::Spent(usage) if usage.input == 10))
  );
  server.finish();
}

#[tokio::test]
async fn anthropic_idle_stream_times_out_and_is_retried() {
  let stalled = vec![
    events::message_start(10, 0, &json!(0)),
    Part::Raw(": keep-alive\n\n".to_owned()),
    Part::Pause(Duration::from_secs(2)),
    events::message_stop(),
  ];
  let script = Script::new()
    .reply_sse(stalled)
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  result.expect("a response after the idle attempt");
  let waits = retries(&seen);
  assert!(
    matches!(waits.as_slice(), [(1, _, ModelError::Transport(text))] if text.contains("sent nothing")),
    "{waits:?}"
  );
  server.finish();
}

#[tokio::test]
async fn anthropic_error_event_mid_stream_is_retried() {
  let failing = vec![
    events::message_start(10, 0, &json!(0)),
    Part::Event(json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}})),
  ];
  let script = Script::new()
    .reply_sse(failing)
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  result.expect("a response after the retry");
  assert!(matches!(retries(&seen).as_slice(), [(1, _, ModelError::Overloaded)]));
  server.finish();
}

#[tokio::test]
async fn anthropic_bound_thinking_is_stripped_and_sent_again() {
  let first = events::response(
    vec![
      events::thinking(0, "Look first.", "sig-a"),
      events::tool_use(
        1,
        "toolu_1",
        "read_file",
        r#"{"path":"a","offset":null,"limit":null}"#,
        true,
      ),
    ],
    "tool_use",
  );
  let no_thinking = |request: &crate::server::Recorded| {
    let body = request.json().to_string();
    assert!(!body.contains("sig-a"), "{body}");
  };
  let script = Script::new()
    .reply_sse(first)
    .reply_error(400, "invalid_request_error", BOUND)
    .history_may_change()
    .expect(no_thinking)
    .reply_sse(events::response(
      vec![events::tool_use(
        0,
        "toolu_2",
        "read_file",
        r#"{"path":"b","offset":null,"limit":null}"#,
        true,
      )],
      "tool_use",
    ))
    .expect(no_thinking)
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let client = client(&server);
  let conversation = ConversationId::generate();
  let mut items = vec![task("Go.")];
  let (first, _) = respond(&client, &request(conversation, items.clone())).await;
  items.extend(first.expect("a first response").items);
  items.push(Item::tool_result(
    ToolCallId::new("toolu_1").expect("an id"),
    "a",
    false,
  ));
  let (second, seen) = respond(&client, &request(conversation, items.clone())).await;
  assert!(seen.contains(&StreamEvent::ThinkingStripped));
  items.extend(second.expect("a response without the old thinking").items);
  items.push(Item::tool_result(
    ToolCallId::new("toolu_2").expect("an id"),
    "b",
    false,
  ));
  let (third, _) = respond(&client, &request(conversation, items)).await;
  third.expect("the conversation goes on without the old thinking");
  server.finish();
}

#[tokio::test]
async fn anthropic_key_never_shows_in_errors() {
  let echoed = format!("invalid x-api-key: {KEY}");
  let script = Script::new().reply_error(400, "invalid_request_error", &echoed);
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let client = client(&server);
  let (result, seen) = respond(&client, &request(ConversationId::generate(), vec![task("Go.")])).await;
  let error = result.expect_err("refused");
  let shown = format!("{error} {error:?} {seen:?} {client:?} {:?}", client.endpoint());
  assert!(!shown.contains(KEY), "{shown}");
  assert!(shown.contains("<redacted>"), "{shown}");
  server.finish();

  let closed = FakeServer::start(Script::new(), events::catalog(), 10).await;
  let unreachable = AnthropicClient::new(endpoint(&closed, WireQuirks::OFFICIAL)).expect("a client");
  drop(closed);
  let (result, seen) = respond(&unreachable, &request(ConversationId::generate(), vec![task("Go.")])).await;
  let error = result.expect_err("unreachable");
  assert!(matches!(error, ModelError::Transport(_)), "{error:?}");
  let shown = format!("{error} {error:?} {seen:?}");
  assert!(!shown.contains(KEY), "{shown}");
}

#[tokio::test]
async fn anthropic_huge_retry_after_is_an_error_not_a_panic() {
  let body = json!({"type": "error", "error": {"type": "rate_limit_error", "message": "slow down"}}).to_string();
  let script = Script::new().reply_status("/v1/messages", 429, &[("retry-after", "1e20")], &body);
  let (error, requests) = one_error(script).await;
  assert_eq!(
    error,
    ModelError::RateLimited {
      retry_after: Some(Duration::MAX)
    }
  );
  assert_eq!(requests, 1);
  for odd in ["-1", "NaN", "inf", "soon"] {
    let script = Script::new()
      .reply_status("/v1/messages", 429, &[("retry-after", odd)], &body)
      .reply_sse(events::text_response("Done."));
    let server = FakeServer::start(script, events::catalog(), 10).await;
    let (result, seen) = respond(
      &client(&server),
      &request(ConversationId::generate(), vec![task("Go.")]),
    )
    .await;
    result.expect("a response after the backoff");
    assert_eq!(
      retries(&seen)[0].2,
      ModelError::RateLimited { retry_after: None },
      "{odd}"
    );
    server.finish();
  }
}

#[tokio::test]
async fn anthropic_retry_after_is_honoured_on_overload() {
  let overloaded = json!({"type": "error", "error": {"type": "overloaded_error", "message": "busy"}}).to_string();
  let script = Script::new()
    .reply_status("/v1/messages", 529, &[("retry-after", "0")], &overloaded)
    .reply_status("/v1/messages", 503, &[("retry-after", "0")], "unavailable")
    .reply_sse(events::text_response("Done."));
  let server = FakeServer::start(script, events::catalog(), 10).await;
  let (result, seen) = respond(
    &client(&server),
    &request(ConversationId::generate(), vec![task("Go.")]),
  )
  .await;
  result.expect("a response after two retries");
  let waits: Vec<(Duration, ModelError)> = retries(&seen)
    .into_iter()
    .map(|(_, wait, error)| (wait, error))
    .collect();
  assert_eq!(
    waits,
    vec![
      (Duration::ZERO, ModelError::Overloaded),
      (Duration::ZERO, ModelError::Server(503))
    ]
  );
  server.finish();
}
