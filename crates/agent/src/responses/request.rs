//! The body of a Responses request (`docs/architecture/native-agent.md` §3.2). It is stateless: nothing
//! is stored at the provider (`store: false`), no `previous_response_id` is sent, and every request
//! carries the whole conversation, reasoning items with their `encrypted_content` included. The same
//! conversation always gives the same bytes: the frozen instructions, tools sorted by name, and the
//! items in order, so the provider's prefix cache (keyed by `prompt_cache_key`) keeps hitting.

use orchester_core::{
  connection::WireApi,
  native::{
    item::Item,
    model::{ModelError, ModelRequest},
    tools::{ToolSet, not_json},
  },
};
use serde_json::{Map, Value, json};

/// What one attempt sends beyond the model request itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Shape {
  pub max_output: u32,
  /// The model reasons: the request names the effort, asks for summaries and for the encrypted
  /// reasoning it has to send back.
  pub reasoning: bool,
  /// Reasoning items among the first this many items are left out (the endpoint could not read them).
  pub strip_before: usize,
}

/// A failed tool's output starts with this, since the wire format has no error flag (§3.3: the error
/// text goes in the result).
const ERROR_PREFIX: &str = "Error: ";

/// The request body.
pub(super) fn body(request: &ModelRequest, shape: Shape) -> Result<Value, ModelError> {
  let mut body = Map::new();
  body.insert("model".to_owned(), json!(request.model.as_str()));
  if !request.system.is_empty() {
    body.insert("instructions".to_owned(), json!(request.system));
  }
  body.insert("tools".to_owned(), tools(&request.tools));
  body.insert("tool_choice".to_owned(), json!("auto"));
  body.insert("parallel_tool_calls".to_owned(), json!(true));
  if shape.reasoning {
    let mut reasoning = json!({"summary": "auto"});
    if let Some(effort) = &request.effort {
      reasoning["effort"] = json!(effort);
    }
    body.insert("reasoning".to_owned(), reasoning);
    body.insert("include".to_owned(), json!(["reasoning.encrypted_content"]));
  }
  body.insert("max_output_tokens".to_owned(), json!(shape.max_output));
  body.insert("store".to_owned(), json!(false));
  body.insert("stream".to_owned(), json!(true));
  body.insert("prompt_cache_key".to_owned(), json!(request.cache_key.to_string()));
  body.insert("input".to_owned(), input(&request.items, shape.strip_before)?);
  Ok(Value::Object(body))
}

/// The function tools, sorted by name so that they never vary within a conversation.
fn tools(set: &ToolSet) -> Value {
  let mut tools = set.tools().to_vec();
  tools.sort_by_key(|tool| tool.as_str());
  let declared = tools.into_iter().map(|tool| {
    let schema: Value = serde_json::from_str(tool.input_schema()).expect("the tool schemas in core are JSON");
    json!({
      "type": "function",
      "name": tool.as_str(),
      "description": tool.description(),
      "parameters": schema,
      "strict": true,
    })
  });
  Value::Array(declared.collect())
}

fn input(items: &[Item], strip_before: usize) -> Result<Value, ModelError> {
  let mut input = Vec::with_capacity(items.len());
  for (index, item) in items.iter().enumerate() {
    if let Some(entry) = entry(item, index < strip_before)? {
      input.push(entry);
    }
  }
  Ok(Value::Array(input))
}

/// The input item of `item`; `None` for what is not sent: empty text, and reasoning that is left out
/// (`strip`), came from another wire format or carries no encrypted content to send back.
fn entry(item: &Item, strip: bool) -> Result<Option<Value>, ModelError> {
  let message = |role: &str, text: &str| json!({"type": "message", "role": role, "content": text});
  let entry = match item {
    Item::User { text, .. } | Item::Compaction { summary: text } if !text.is_empty() => message("user", text),
    Item::Assistant { text } if !text.is_empty() => message("assistant", text),
    Item::User { .. } | Item::Compaction { .. } | Item::Assistant { .. } => return Ok(None),
    Item::ToolCall { id, name, input } => json!({
      "type": "function_call",
      "call_id": id.as_str(),
      "name": name,
      "arguments": arguments(input),
    }),
    Item::ToolResult { id, content, is_error } => {
      let output = if *is_error {
        format!("{ERROR_PREFIX}{content}")
      } else {
        content.clone()
      };
      json!({"type": "function_call_output", "call_id": id.as_str(), "output": output})
    }
    Item::Reasoning { opaque, .. } if !strip && opaque.wire() == WireApi::OpenAiResponses => {
      let stored: Value = serde_json::from_str(opaque.json())
        .map_err(|error| ModelError::Protocol(format!("a stored reasoning item is not JSON: {error}")))?;
      return Ok(reasoning(&stored));
    }
    Item::Reasoning { .. } => return Ok(None),
  };
  Ok(Some(entry))
}

/// A stored reasoning item as it is sent back: its summary and encrypted content, without its id and
/// status, which name an item the provider did not keep (`store: false`). The Codex CLI sends its
/// reasoning items back the same way.
fn reasoning(stored: &Value) -> Option<Value> {
  let encrypted = stored.get("encrypted_content").filter(|value| value.is_string())?;
  let summary = stored.get("summary").cloned().unwrap_or_else(|| json!([]));
  Some(json!({"type": "reasoning", "summary": summary, "encrypted_content": encrypted}))
}

/// A call's arguments as the JSON text the model wrote: the raw text of input that was not JSON,
/// else the input serialized.
fn arguments(input: &Value) -> String {
  not_json(input).map_or_else(|| input.to_string(), str::to_owned)
}
