//! Requests and responses on the wire: the stateless request's shape, a streamed turn read into items,
//! the append-only history with reasoning sent back and its cache reads, invalid call arguments, the
//! output limit, refusals and the content filter.

use orchester_core::{
  id::ConversationId,
  native::{
    item::{Item, ToolCallId},
    model::{StopReason, StreamEvent},
    tools::{ToolInputError, parse_call},
  },
  resource::ModelId,
  run::TokenUsage,
};
use serde_json::{Value, json};

use crate::{
  KEY, client,
  events::{self, MODEL},
  request, respond, respond_once,
  server::{Part, Script},
  start, task,
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
async fn responses_request_is_stateless_with_the_designed_shape() {
  let script = Script::new()
    .expect(|request| {
      assert_eq!(request.header("authorization"), Some(format!("Bearer {KEY}").as_str()));
      let body = request.json();
      assert_eq!(body["model"], json!(MODEL));
      assert_eq!(body["instructions"], json!("You work on one step of a task."));
      assert_eq!(body["stream"], json!(true));
      assert_eq!(body["store"], json!(false));
      assert!(body.get("previous_response_id").is_none());
      assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
      assert_eq!(body["reasoning"], json!({"effort": "high", "summary": "auto"}));
      assert_eq!(body["max_output_tokens"], json!(64_000));
      assert_eq!(body["tool_choice"], json!("auto"));
      assert_eq!(body["parallel_tool_calls"], json!(true));
      assert!(body["prompt_cache_key"].as_str().is_some_and(|key| !key.is_empty()));
      let names = tool_names(&body);
      let mut sorted = names.clone();
      sorted.sort();
      assert_eq!(names, sorted);
      for tool in body["tools"].as_array().expect("tools") {
        assert_eq!(tool["type"], json!("function"));
        assert_eq!(tool["strict"], json!(true));
        assert_eq!(tool["parameters"]["additionalProperties"], json!(false));
      }
      assert_eq!(
        body["input"],
        json!([{"type": "message", "role": "user", "content": "Fix the bug."}])
      );
    })
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
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
  assert_eq!(response.served_by.as_str(), MODEL);
  assert!(seen.contains(&StreamEvent::Text("Do".to_owned())));
  server.finish();
}

#[tokio::test]
async fn responses_streamed_turn_becomes_items_with_usage() {
  let parts = events::response(
    vec![
      events::reasoning(0, "Plan the read.", "enc-1"),
      events::message(1, "Reading."),
      events::function_call(2, "call_1", "read_file", READ_INPUT, true),
    ],
    events::completed(60, &json!(30), 7),
  );
  let server = start(Script::new().reply_sse(parts)).await;
  let (result, seen) = respond_once(&server).await;
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
  let [
    Item::Reasoning { summary, opaque },
    Item::Assistant { text },
    Item::ToolCall { id, name, input },
  ] = response.items.as_slice()
  else {
    panic!("unexpected items: {:?}", response.items);
  };
  assert_eq!(summary.as_deref(), Some("Plan the read."));
  let item: Value = serde_json::from_str(opaque.json()).expect("JSON");
  assert_eq!(item, events::reasoning_item(0, "Plan the read.", "enc-1"));
  assert_eq!(text, "Reading.");
  assert_eq!(id, &ToolCallId::new("call_1").expect("an id"));
  assert_eq!(name, "read_file");
  assert_eq!(input, &serde_json::from_str::<Value>(READ_INPUT).expect("JSON"));
  assert!(seen.contains(&StreamEvent::Reasoning("Plan the read.".to_owned())));
  assert!(seen.contains(&StreamEvent::ToolCallStarted {
    name: "read_file".to_owned()
  }));
  server.finish();
}

#[tokio::test]
async fn responses_history_is_append_only_resends_reasoning_and_hits_the_cache() {
  let first = events::response(
    vec![
      events::reasoning(0, "Look first.", "enc-a"),
      events::function_call(1, "call_1", "read_file", READ_INPUT, true),
    ],
    events::completed(10, &json!(0), 5),
  );
  let second = events::response(
    vec![events::message(0, "Done.")],
    events::completed(40, &json!("echo"), 3),
  );
  let script = Script::new()
    .reply_sse(first)
    .expect(|request| {
      let input = request.json()["input"].clone();
      assert_eq!(
        input[1],
        json!({"type": "reasoning", "summary": [{"type": "summary_text", "text": "Look first."}],
          "encrypted_content": "enc-a"})
      );
      assert_eq!(
        input[2],
        json!({"type": "function_call", "call_id": "call_1", "name": "read_file",
          "arguments": serde_json::from_str::<Value>(READ_INPUT).expect("JSON").to_string()})
      );
      assert_eq!(
        input[3],
        json!({"type": "function_call_output", "call_id": "call_1", "output": "1\tfn main() {}"})
      );
    })
    .reply_sse(second);
  let server = start(script).await;
  let client = client(&server);
  let conversation = ConversationId::generate();
  let mut items = vec![task("Fix it.")];
  let (first, _) = respond(&client, &request(conversation, items.clone())).await;
  items.extend(first.expect("a first response").items);
  items.push(Item::tool_result(
    ToolCallId::new("call_1").expect("an id"),
    "1\tfn main() {}",
    false,
  ));
  let (second, _) = respond(&client, &request(conversation, items)).await;
  assert!(second.expect("a second response").usage.cached_input > 0);
  server.finish();
}

#[tokio::test]
async fn responses_failed_tool_output_says_error() {
  let script = Script::new()
    .expect(|request| {
      assert_eq!(
        request.json()["input"][1],
        json!({"type": "function_call_output", "call_id": "call_1", "output": "Error: no such file"})
      );
    })
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let items = vec![
    task("Go."),
    Item::tool_result(ToolCallId::new("call_1").expect("an id"), "no such file", true),
  ];
  let (result, _) = respond(&client(&server), &request(ConversationId::generate(), items)).await;
  result.expect("a response");
  server.finish();
}

#[tokio::test]
async fn responses_invalid_arguments_are_kept_and_sent_back_as_written() {
  let raw = r#"{"path": "a.rs", "offset": nul"#;
  let parts = events::response(
    vec![events::function_call(0, "call_9", "read_file", raw, true)],
    events::completed(10, &json!(0), 5),
  );
  let script = Script::new()
    .reply_sse(parts)
    .expect(move |request| assert_eq!(request.json()["input"][1]["arguments"], json!(raw)))
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let client = client(&server);
  let conversation = ConversationId::generate();
  let (result, _) = respond(&client, &request(conversation, vec![task("Go.")])).await;
  let response = result.expect("a response");
  let [Item::ToolCall { id, name, input }] = response.items.as_slice() else {
    panic!("unexpected items: {:?}", response.items);
  };
  assert_eq!(input, &json!({"INVALID_JSON": raw}));
  let error = parse_call(name, input).expect_err("not JSON");
  assert_eq!(error, ToolInputError::NotJson(raw.to_owned()));
  let mut items = vec![task("Go.")];
  items.extend(response.items.clone());
  items.push(Item::tool_result(id.clone(), error.to_string(), true));
  respond(&client, &request(conversation, items))
    .await
    .0
    .expect("a second response");
  server.finish();
}

#[tokio::test]
async fn responses_output_limit_inside_a_call_asks_again_with_room() {
  let cut = events::response(
    vec![events::function_call(0, "call_1", "read_file", READ_INPUT, false)],
    events::incomplete("max_output_tokens"),
  );
  let whole = events::response(
    vec![events::function_call(0, "call_1", "read_file", READ_INPUT, true)],
    events::completed(10, &json!(0), 5),
  );
  let script = Script::new()
    .expect(|request| assert_eq!(request.json()["max_output_tokens"], json!(64_000)))
    .reply_sse(cut)
    .expect(|request| assert_eq!(request.json()["max_output_tokens"], json!(128_000)))
    .reply_sse(whole);
  let server = start(script).await;
  let (result, seen) = respond_once(&server).await;
  assert_eq!(result.expect("a response").stop, StopReason::ToolUse);
  assert!(
    seen
      .iter()
      .any(|event| matches!(event, StreamEvent::Spent(usage) if usage.output == 5))
  );
  server.finish();
}

#[tokio::test]
async fn responses_output_limit_without_room_keeps_the_cut_arguments() {
  let cut = events::response(
    vec![events::function_call(0, "call_1", "read_file", READ_INPUT, false)],
    events::incomplete("max_output_tokens"),
  );
  let server = start(Script::new().reply_sse(cut)).await;
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
async fn responses_refusals_and_the_content_filter_stop_the_turn() {
  let refused = events::response(
    vec![events::refusal(0, "I can't help with that.")],
    events::completed(10, &json!(0), 4),
  );
  let policy = vec![
    events::created(),
    Part::Event(
      json!({"type": "response.failed", "response": {"id": "resp_1", "status": "failed",
      "error": {"code": "bio_policy", "message": "blocked"}, "usage": null}}),
    ),
  ];
  let filtered = events::response(vec![events::message(0, "Par")], events::incomplete("content_filter"));
  let cases = [
    (refused, StopReason::Refusal { category: None }),
    (
      policy,
      StopReason::Refusal {
        category: Some("bio_policy".to_owned()),
      },
    ),
    (filtered, StopReason::Filtered),
  ];
  for (parts, expected) in cases {
    let server = start(Script::new().reply_sse(parts)).await;
    let (result, _) = respond_once(&server).await;
    assert_eq!(result.expect("a response").stop, expected);
    server.finish();
  }
}

#[tokio::test]
async fn responses_model_outside_the_table_reasons_only_when_asked() {
  let script = Script::new()
    .expect(|request| {
      let body = request.json();
      assert!(body.get("reasoning").is_none());
      assert!(body.get("include").is_none());
      assert_eq!(body["max_output_tokens"], json!(16_000));
    })
    .reply_sse(events::text_response("Done."));
  let server = start(script).await;
  let mut asked = request(ConversationId::generate(), vec![task("Go.")]);
  asked.model = ModelId::new("lab-chat-model").expect("a model id");
  asked.effort = None;
  asked.max_output = 16_000;
  respond(&client(&server), &asked).await.0.expect("a response");
  server.finish();
}

#[tokio::test]
#[should_panic(expected = "the history was edited")]
async fn responses_fake_server_catches_an_edited_history() {
  let script = Script::new()
    .reply_sse(events::text_response("One."))
    .reply_sse(events::text_response("Two."));
  let server = start(script).await;
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
