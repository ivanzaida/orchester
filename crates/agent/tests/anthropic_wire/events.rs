//! Messages stream events for scripted turns, shaped as the API streams them.

use serde_json::{Value, json};

use crate::server::Part;

pub const MODEL: &str = "claude-opus-5-5";

/// A model object as the Models API returns it.
pub fn model(id: &str, adaptive: bool, max_tokens: u64, efforts: &[&str]) -> Value {
  let mut effort = json!({"supported": !efforts.is_empty()});
  for level in ["low", "medium", "high", "xhigh", "max"] {
    effort[level] = json!({"supported": efforts.contains(&level)});
  }
  json!({
    "id": id,
    "display_name": id,
    "max_input_tokens": 1_000_000,
    "max_tokens": max_tokens,
    "capabilities": {
      "thinking": {"supported": true, "types": {"adaptive": {"supported": adaptive}, "enabled": {"supported": false}}},
      "effort": effort,
    },
  })
}

/// The default catalog: Opus 5.5 with adaptive thinking and 128K output.
pub fn catalog() -> Vec<Value> {
  vec![model(MODEL, true, 128_000, &["low", "medium", "high", "xhigh", "max"])]
}

pub fn message_start(input: u64, cache_creation: u64, cache_read: &Value) -> Part {
  Part::Event(json!({
    "type": "message_start",
    "message": {
      "id": "msg_1", "type": "message", "role": "assistant", "model": MODEL, "content": [],
      "usage": {"input_tokens": input, "cache_creation_input_tokens": cache_creation,
        "cache_read_input_tokens": cache_read, "output_tokens": 1},
    },
  }))
}

pub fn text(index: u64, text: &str) -> Vec<Part> {
  vec![
    Part::Event(json!({"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}})),
    Part::Event(json!({"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": text}})),
    Part::Event(json!({"type": "content_block_stop", "index": index})),
  ]
}

pub fn thinking(index: u64, summary: &str, signature: &str) -> Vec<Part> {
  vec![
    Part::Event(json!({"type": "content_block_start", "index": index,
      "content_block": {"type": "thinking", "thinking": "", "signature": ""}})),
    Part::Event(json!({"type": "content_block_delta", "index": index,
      "delta": {"type": "thinking_delta", "thinking": summary}})),
    Part::Event(json!({"type": "content_block_delta", "index": index,
      "delta": {"type": "signature_delta", "signature": signature}})),
    Part::Event(json!({"type": "content_block_stop", "index": index})),
  ]
}

/// A tool call whose input streams in two pieces; `finished` sends its `content_block_stop`.
pub fn tool_use(index: u64, id: &str, name: &str, input: &str, finished: bool) -> Vec<Part> {
  let (first, second) = input.split_at(input.len() / 2);
  let mut parts = vec![Part::Event(json!({"type": "content_block_start", "index": index,
    "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}}))];
  for piece in [first, second] {
    parts.push(Part::Event(json!({"type": "content_block_delta", "index": index,
      "delta": {"type": "input_json_delta", "partial_json": piece}})));
  }
  if finished {
    parts.push(Part::Event(json!({"type": "content_block_stop", "index": index})));
  }
  parts
}

pub fn message_delta(stop_reason: &str, output: u64) -> Part {
  Part::Event(
    json!({"type": "message_delta", "delta": {"stop_reason": stop_reason, "stop_details": null},
    "usage": {"output_tokens": output}}),
  )
}

pub fn message_stop() -> Part {
  Part::Event(json!({"type": "message_stop"}))
}

/// A whole response: start, `blocks`, the stop and its end.
pub fn response(blocks: Vec<Vec<Part>>, stop_reason: &str) -> Vec<Part> {
  let mut parts = vec![message_start(10, 0, &json!(0))];
  parts.extend(blocks.into_iter().flatten());
  parts.push(message_delta(stop_reason, 5));
  parts.push(message_stop());
  parts
}

/// A response with one text block.
pub fn text_response(body: &str) -> Vec<Part> {
  response(vec![text(0, body)], "end_turn")
}
