//! Failures: status codes and OpenAI's error codes mapped to typed errors, retries with backoff and
//! `retry-after`, broken and idle streams, errors in the stream, the recovery from encrypted reasoning
//! the endpoint cannot read, and the API key kept out of every error.

use std::time::Duration;

use orchester_agent::ResponsesClient;
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
  request, respond, respond_once,
  server::{Part, Recorded, Script},
  start, task,
};

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
  let server = start(script).await;
  let (result, _) = respond_once(&server).await;
  let error = result.expect_err("an error");
  let count = server.model_requests().len();
  server.finish();
  (error, count)
}

fn error_body(kind: &str, code: &str, message: &str) -> String {
  json!({"error": {"type": kind, "code": code, "message": message, "param": null}}).to_string()
}

#[tokio::test]
async fn responses_statuses_and_codes_map_to_typed_errors() {
  let model = ModelId::new(MODEL).expect("a model id");
  let cases = [
    (401, "invalid_api_key", "bad key", ModelError::KeyRejected),
    (402, "billing_error", "no credit", ModelError::Billing),
    (
      429,
      "insufficient_quota",
      "You exceeded your current quota",
      ModelError::Billing,
    ),
    (
      403,
      "unsupported_country_region_territory",
      "not here",
      ModelError::Forbidden,
    ),
    (
      404,
      "model_not_found",
      "no such model",
      ModelError::ModelNotFound(model),
    ),
    (413, "request_too_large", "too large", ModelError::TooLarge),
    (
      400,
      "context_length_exceeded",
      "Your input exceeds the context window",
      ModelError::TooLarge,
    ),
    (
      400,
      "invalid_value",
      "input[3]: bad field",
      ModelError::InvalidRequest("input[3]: bad field".to_owned()),
    ),
  ];
  for (status, code, message, expected) in cases {
    let (error, requests) = one_error(Script::new().reply_error(status, code, message)).await;
    assert_eq!(error, expected, "status {status} {code}");
    assert_eq!(requests, 1, "status {status} {code} is not retried");
  }
}

#[tokio::test]
async fn responses_rate_limit_waits_as_retry_after_says() {
  let body = error_body("requests", "rate_limit_exceeded", "slow down");
  let script = Script::new()
    .reply_status("/v1/responses", 429, &[("retry-after", "0")], &body)
    .reply_status(
      "/v1/responses",
      429,
      &[("retry-after-ms", "20"), ("retry-after", "999")],
      &body,
    )
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let (result, seen) = respond_once(&server).await;
  result.expect("a response after the retries");
  let limited = |after: u64| ModelError::RateLimited {
    retry_after: Some(Duration::from_millis(after)),
  };
  assert_eq!(
    retries(&seen),
    vec![
      (1, Duration::ZERO, limited(0)),
      (2, Duration::from_millis(20), limited(20))
    ]
  );
  server.finish();
}

#[tokio::test]
async fn responses_server_errors_are_retried_with_backoff_and_end_after_five() {
  let script = Script::new()
    .reply_error(500, "server_error", "internal")
    .reply_error(503, "server_error", "unavailable")
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let (result, seen) = respond_once(&server).await;
  result.expect("a response after two retries");
  let waits = retries(&seen);
  assert_eq!((waits[0].0, &waits[0].2), (1, &ModelError::Server(500)));
  assert_eq!((waits[1].0, &waits[1].2), (2, &ModelError::Server(503)));
  assert!(waits[1].1 >= Duration::from_millis(10), "{:?}", waits[1].1);
  server.finish();

  let mut script = Script::new();
  for _ in 0..6 {
    script = script.reply_error(502, "server_error", "bad gateway");
  }
  let server = start(script).await;
  let (result, seen) = respond_once(&server).await;
  assert_eq!(result.expect_err("down"), ModelError::Server(502));
  assert_eq!(retries(&seen).len(), 5);
  assert_eq!(server.model_requests().len(), 6);
  server.finish();
}

#[tokio::test]
async fn responses_huge_or_odd_retry_after_is_an_error_or_a_backoff() {
  let body = error_body("requests", "rate_limit_exceeded", "slow down");
  let script = Script::new().reply_status("/v1/responses", 429, &[("retry-after", "1e20")], &body);
  let (error, requests) = one_error(script).await;
  assert_eq!(
    error,
    ModelError::RateLimited {
      retry_after: Some(Duration::MAX)
    }
  );
  assert_eq!(requests, 1);
  for odd in ["-1", "NaN", "soon"] {
    let script = Script::new()
      .reply_status("/v1/responses", 429, &[("retry-after-ms", odd)], &body)
      .reply_sse(events::text_response("Done."));
    let server = start(script).await;
    let (result, seen) = respond_once(&server).await;
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
async fn responses_broken_stream_is_dropped_and_retried() {
  let mut broken = vec![events::created()];
  broken.extend(events::message(0, "Half").into_iter().take(3));
  broken.push(Part::Drop);
  let script = Script::new()
    .reply_sse(broken)
    .reply_sse(events::text_response("Whole."));
  let server = start(script).await;
  let (result, seen) = respond_once(&server).await;
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
  server.finish();
}

#[tokio::test]
async fn responses_idle_stream_times_out_and_is_retried() {
  let stalled = vec![
    events::created(),
    Part::Raw(": keep-alive\n\n".to_owned()),
    Part::Pause(Duration::from_secs(2)),
    events::completed(10, &json!(0), 5),
  ];
  let script = Script::new()
    .reply_sse(stalled)
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let (result, seen) = respond_once(&server).await;
  result.expect("a response after the idle attempt");
  let waits = retries(&seen);
  assert!(
    matches!(waits.as_slice(), [(1, _, ModelError::Transport(text))] if text.contains("sent nothing")),
    "{waits:?}"
  );
  server.finish();
}

#[tokio::test]
async fn responses_errors_in_the_stream_are_retried() {
  let error_event = vec![
    events::created(),
    Part::Event(json!({"type": "error", "code": "rate_limit_exceeded", "message": "slow down", "param": null})),
  ];
  let failed = vec![
    events::created(),
    Part::Event(
      json!({"type": "response.failed", "response": {"id": "resp_1", "status": "failed",
      "error": {"code": "server_error", "message": "internal"}}}),
    ),
  ];
  let script = Script::new()
    .reply_sse(error_event)
    .reply_sse(failed)
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let (result, seen) = respond_once(&server).await;
  result.expect("a response after the retries");
  let errors: Vec<ModelError> = retries(&seen).into_iter().map(|(_, _, error)| error).collect();
  assert_eq!(
    errors,
    vec![ModelError::RateLimited { retry_after: None }, ModelError::Server(500)]
  );
  server.finish();
}

#[tokio::test]
async fn responses_unreadable_reasoning_is_left_out_and_sent_again() {
  let first = events::response(
    vec![
      events::reasoning(0, "Look first.", "enc-a"),
      events::function_call(
        1,
        "call_1",
        "read_file",
        r#"{"path":"a","offset":null,"limit":null}"#,
        true,
      ),
    ],
    events::completed(10, &json!(0), 5),
  );
  let second = events::response(
    vec![events::function_call(
      0,
      "call_2",
      "read_file",
      r#"{"path":"b","offset":null,"limit":null}"#,
      true,
    )],
    events::completed(10, &json!(0), 5),
  );
  let no_reasoning = |request: &Recorded| {
    let body = request.json().to_string();
    assert!(!body.contains("enc-a"), "{body}");
  };
  let script = Script::new()
    .reply_sse(first)
    .reply_error(
      400,
      "invalid_encrypted_content",
      "The encrypted content for item rs_0 could not be verified.",
    )
    .history_may_change()
    .expect(no_reasoning)
    .reply_sse(second)
    .expect(no_reasoning)
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let client = client(&server);
  let conversation = ConversationId::generate();
  let mut items = vec![task("Go.")];
  let (first, _) = respond(&client, &request(conversation, items.clone())).await;
  items.extend(first.expect("a first response").items);
  items.push(Item::tool_result(ToolCallId::new("call_1").expect("an id"), "a", false));
  let (second, seen) = respond(&client, &request(conversation, items.clone())).await;
  assert!(seen.contains(&StreamEvent::ThinkingStripped));
  items.extend(second.expect("a response without the old reasoning").items);
  items.push(Item::tool_result(ToolCallId::new("call_2").expect("an id"), "b", false));
  let (third, _) = respond(&client, &request(conversation, items)).await;
  third.expect("the conversation goes on without the old reasoning");
  server.finish();
}

#[tokio::test]
async fn responses_key_never_shows_in_errors() {
  let echoed = format!("Incorrect API key provided: {KEY}");
  let script = Script::new().reply_error(400, "invalid_request_error", &echoed);
  let server = start(script).await;
  let client = client(&server);
  let (result, seen) = respond(&client, &request(ConversationId::generate(), vec![task("Go.")])).await;
  let error = result.expect_err("refused");
  let shown = format!("{error} {error:?} {seen:?} {client:?} {:?}", client.endpoint());
  assert!(!shown.contains(KEY), "{shown}");
  assert!(shown.contains("<redacted>"), "{shown}");
  server.finish();

  let closed = start(Script::new()).await;
  let unreachable = ResponsesClient::new(endpoint(&closed)).expect("a client");
  drop(closed);
  let (result, seen) = respond(&unreachable, &request(ConversationId::generate(), vec![task("Go.")])).await;
  let error = result.expect_err("unreachable");
  assert!(matches!(error, ModelError::Transport(_)), "{error:?}");
  let shown = format!("{error} {error:?} {seen:?}");
  assert!(!shown.contains(KEY), "{shown}");
}
