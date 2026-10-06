//! Responses stream events for scripted turns, shaped as the API streams them.

use serde_json::{Value, json};

use crate::server::Part;

/// A model of the built-in table: 128K output, efforts `low` to `max`.
pub const MODEL: &str = "gpt-6.1-sol";

fn event(value: Value) -> Part {
  Part::Event(value)
}

pub fn created() -> Part {
  event(json!({"type": "response.created", "sequence_number": 0,
    "response": {"id": "resp_1", "object": "response", "status": "in_progress", "model": MODEL, "output": []}}))
}

/// A message with `text`, streamed in two deltas.
pub fn message(index: u64, text: &str) -> Vec<Part> {
  let (first, second) = text.split_at(text.len() / 2);
  let id = format!("msg_{index}");
  let mut parts = vec![
    event(json!({"type": "response.output_item.added", "output_index": index,
      "item": {"type": "message", "id": id, "role": "assistant", "status": "in_progress", "content": []}})),
    event(
      json!({"type": "response.content_part.added", "output_index": index, "item_id": id, "content_index": 0,
      "part": {"type": "output_text", "text": "", "annotations": []}}),
    ),
  ];
  for piece in [first, second] {
    parts.push(event(
      json!({"type": "response.output_text.delta", "output_index": index, "item_id": id,
      "content_index": 0, "delta": piece}),
    ));
  }
  parts.extend([
    event(
      json!({"type": "response.output_text.done", "output_index": index, "item_id": id, "content_index": 0,
      "text": text}),
    ),
    event(json!({"type": "response.output_item.done", "output_index": index,
      "item": {"type": "message", "id": id, "role": "assistant", "status": "completed",
        "content": [{"type": "output_text", "text": text, "annotations": []}]}})),
  ]);
  parts
}

/// A message that refuses with `text`.
pub fn refusal(index: u64, text: &str) -> Vec<Part> {
  vec![
    event(json!({"type": "response.output_item.added", "output_index": index,
      "item": {"type": "message", "id": "msg_r", "role": "assistant", "status": "in_progress", "content": []}})),
    event(
      json!({"type": "response.refusal.delta", "output_index": index, "item_id": "msg_r", "content_index": 0,
      "delta": text}),
    ),
    event(json!({"type": "response.output_item.done", "output_index": index,
      "item": {"type": "message", "id": "msg_r", "role": "assistant", "status": "completed",
        "content": [{"type": "refusal", "refusal": text}]}})),
  ]
}

/// The reasoning item `reasoning` makes, as `response.output_item.done` gives it.
pub fn reasoning_item(index: u64, summary: &str, encrypted: &str) -> Value {
  json!({"type": "reasoning", "id": format!("rs_{index}"), "status": "completed",
    "summary": [{"type": "summary_text", "text": summary}], "encrypted_content": encrypted})
}

/// A reasoning item with a summary and its encrypted content.
pub fn reasoning(index: u64, summary: &str, encrypted: &str) -> Vec<Part> {
  let id = format!("rs_{index}");
  vec![
    event(json!({"type": "response.output_item.added", "output_index": index,
      "item": {"type": "reasoning", "id": id, "summary": []}})),
    event(
      json!({"type": "response.reasoning_summary_part.added", "output_index": index, "item_id": id,
      "summary_index": 0, "part": {"type": "summary_text", "text": ""}}),
    ),
    event(
      json!({"type": "response.reasoning_summary_text.delta", "output_index": index, "item_id": id,
      "summary_index": 0, "delta": summary}),
    ),
    event(json!({"type": "response.output_item.done", "output_index": index,
      "item": reasoning_item(index, summary, encrypted)})),
  ]
}

/// A function call whose arguments stream in two pieces; `finished` sends its end.
pub fn function_call(index: u64, call_id: &str, name: &str, arguments: &str, finished: bool) -> Vec<Part> {
  let (first, second) = arguments.split_at(arguments.len() / 2);
  let id = format!("fc_{index}");
  let mut parts = vec![event(
    json!({"type": "response.output_item.added", "output_index": index,
    "item": {"type": "function_call", "id": id, "call_id": call_id, "name": name, "arguments": "",
      "status": "in_progress"}}),
  )];
  for piece in [first, second] {
    parts.push(event(
      json!({"type": "response.function_call_arguments.delta", "output_index": index,
      "item_id": id, "delta": piece}),
    ));
  }
  if finished {
    parts.extend([
      event(
        json!({"type": "response.function_call_arguments.done", "output_index": index, "item_id": id,
        "arguments": arguments}),
      ),
      event(json!({"type": "response.output_item.done", "output_index": index,
        "item": {"type": "function_call", "id": id, "call_id": call_id, "name": name, "arguments": arguments,
          "status": "completed"}})),
    ]);
  }
  parts
}

fn usage(input: u64, cached: &Value, output: u64) -> Value {
  json!({"input_tokens": input, "input_tokens_details": {"cached_tokens": cached},
    "output_tokens": output, "output_tokens_details": {"reasoning_tokens": 2}, "total_tokens": input + output})
}

/// `response.completed` with its usage; `cached` may be `"echo"`.
pub fn completed(input: u64, cached: &Value, output: u64) -> Part {
  event(
    json!({"type": "response.completed", "response": {"id": "resp_1", "status": "completed",
    "model": MODEL, "incomplete_details": null, "usage": usage(input, cached, output)}}),
  )
}

/// `response.incomplete` for `reason`.
pub fn incomplete(reason: &str) -> Part {
  event(
    json!({"type": "response.incomplete", "response": {"id": "resp_1", "status": "incomplete",
    "model": MODEL, "incomplete_details": {"reason": reason}, "usage": usage(10, &json!(0), 5)}}),
  )
}

/// A whole response: created, `outputs`, then `end`.
pub fn response(outputs: Vec<Vec<Part>>, end: Part) -> Vec<Part> {
  let mut parts = vec![created()];
  parts.extend(outputs.into_iter().flatten());
  parts.push(end);
  parts
}

/// A completed response with one message.
pub fn text_response(body: &str) -> Vec<Part> {
  response(vec![message(0, body)], completed(10, &json!(0), 5))
}

/// A model as `GET /models` lists it.
pub fn listed(id: &str) -> Value {
  json!({"id": id, "object": "model", "created": 1_780_000_000, "owned_by": "system"})
}

/// The default catalog: the table's model and one the table does not know.
pub fn catalog() -> Vec<Value> {
  vec![listed(MODEL), listed("ft:gpt-6-luna:lab::abc123")]
}
