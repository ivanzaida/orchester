//! The body of a Messages request (`docs/architecture/native-agent.md` §3.1). The same conversation
//! always gives the same bytes: tools sorted by name, the frozen system prompt with a cache breakpoint,
//! automatic caching for the growing tail, and the items in order, each response's blocks as they
//! streamed (thinking blocks verbatim) and every result of a response in one user message.

use orchester_core::{
  attachment::MediaType,
  connection::WireApi,
  native::{
    item::Item,
    model::{ModelError, ModelRequest},
    tools::ToolSet,
  },
};
use serde_json::{Map, Value, json};

use crate::endpoint::Endpoint;

/// The API version every request names.
pub(super) const VERSION: &str = "2023-06-01";
/// Lets the request say how a thinking block bound to another history is handled.
pub(super) const BETA_BINDING: &str = "thinking-binding-controls-2026-08-01";
/// Lets a declined request run again on another model.
pub(super) const BETA_FALLBACK: &str = "server-side-fallback-2026-07-01";

/// What one attempt sends beyond the model request itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Shape {
  pub max_tokens: u32,
  /// The model thinks adaptively; the request asks for summaries of it.
  pub thinking: bool,
  /// Thinking blocks among the first this many items are left out (§3.1, the preserved-thinking
  /// recovery).
  pub strip_before: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
  User,
  Assistant,
}

impl Role {
  const fn as_str(self) -> &'static str {
    match self {
      Self::User => "user",
      Self::Assistant => "assistant",
    }
  }
}

/// The `anthropic-beta` values the request needs.
pub(super) fn betas(shape: Shape, endpoint: &Endpoint) -> Vec<&'static str> {
  let mut betas = Vec::new();
  if endpoint.quirks().betas && shape.thinking {
    betas.push(BETA_BINDING);
  }
  if endpoint.provider_fallback() {
    betas.push(BETA_FALLBACK);
  }
  betas
}

/// The request body.
pub(super) fn body(request: &ModelRequest, shape: Shape, endpoint: &Endpoint) -> Result<Value, ModelError> {
  let quirks = endpoint.quirks();
  let mut body = Map::new();
  body.insert("model".to_owned(), json!(request.model.as_str()));
  body.insert("max_tokens".to_owned(), json!(shape.max_tokens));
  body.insert("stream".to_owned(), json!(true));
  if !request.system.is_empty() {
    let system = if quirks.cache_control {
      json!([{"type": "text", "text": request.system, "cache_control": {"type": "ephemeral"}}])
    } else {
      json!(request.system)
    };
    body.insert("system".to_owned(), system);
  }
  if quirks.cache_control {
    body.insert("cache_control".to_owned(), json!({"type": "ephemeral"}));
  }
  // A request that offers no tools (auto mode's review) sends none.
  if !request.tools.tools().is_empty() {
    body.insert("tools".to_owned(), tools(&request.tools, quirks.eager_input_streaming));
  }
  if shape.thinking {
    let mut thinking = json!({"type": "adaptive", "display": "summarized"});
    if quirks.betas {
      thinking["block_binding"] = json!({"prefix_mismatch_behavior": "error"});
    }
    body.insert("thinking".to_owned(), thinking);
  }
  if let Some(effort) = &request.effort {
    body.insert("output_config".to_owned(), json!({"effort": effort}));
  }
  if endpoint.provider_fallback() {
    body.insert("fallbacks".to_owned(), json!("default"));
  }
  body.insert("messages".to_owned(), messages(&request.items, shape.strip_before)?);
  Ok(Value::Object(body))
}

/// The tool declarations, sorted by name so that they never vary within a conversation.
fn tools(set: &ToolSet, eager: bool) -> Value {
  let mut tools = set.tools().to_vec();
  tools.sort_by_key(|tool| tool.as_str());
  let declared = tools.into_iter().map(|tool| {
    let schema: Value = serde_json::from_str(tool.input_schema()).expect("the tool schemas in core are JSON");
    let mut declaration = json!({
      "name": tool.as_str(),
      "description": tool.description(),
      "input_schema": schema,
      "strict": true,
    });
    if eager {
      declaration["eager_input_streaming"] = json!(true);
    }
    declaration
  });
  Value::Array(declared.collect())
}

/// The conversation as messages: consecutive items of one side make one message.
fn messages(items: &[Item], strip_before: usize) -> Result<Value, ModelError> {
  let mut messages: Vec<(Role, Vec<Value>)> = Vec::new();
  for (index, item) in items.iter().enumerate() {
    let Some((role, block)) = block(item, index < strip_before)? else {
      continue;
    };
    let blocks = std::iter::once(block).chain(attached_block(item));
    match messages.last_mut() {
      Some((last, held)) if *last == role => held.extend(blocks),
      _ => messages.push((role, blocks.collect())),
    }
  }
  let messages = messages
    .into_iter()
    .map(|(role, content)| json!({"role": role.as_str(), "content": content}));
  Ok(Value::Array(messages.collect()))
}

/// The image or document block of a file a user item gives, after its line; a copy that cannot be read is said in
/// a text block instead, so the model is not told of a file it never sees.
fn attached_block(item: &Item) -> Option<Value> {
  let Item::User {
    attached: Some(file), ..
  } = item
  else {
    return None;
  };
  let media = file.attachment.media();
  let kind = if media == MediaType::Pdf { "document" } else { "image" };
  Some(match file.base64() {
    Ok(data) => json!({"type": kind, "source": {"type": "base64", "media_type": media.as_str(), "data": data}}),
    Err(_) => json!({"type": "text", "text": UNREADABLE}),
  })
}

/// What the model reads in place of an attached file whose copy cannot be read.
const UNREADABLE: &str = "The attachment above could not be read from the project's data.";

/// The content block of `item`; `None` for what is not sent: empty text, and thinking that is left out
/// (`strip`) or came from another wire format.
fn block(item: &Item, strip: bool) -> Result<Option<(Role, Value)>, ModelError> {
  let text = |text: &str| json!({"type": "text", "text": text});
  let block = match item {
    Item::User { text: body, .. } | Item::Compaction { summary: body } if !body.is_empty() => (Role::User, text(body)),
    Item::Assistant { text: body } if !body.is_empty() => (Role::Assistant, text(body)),
    Item::User { .. } | Item::Compaction { .. } | Item::Assistant { .. } => return Ok(None),
    Item::ToolResult { id, content, is_error } => {
      let mut result = json!({"type": "tool_result", "tool_use_id": id.as_str(), "content": content});
      if *is_error {
        result["is_error"] = json!(true);
      }
      (Role::User, result)
    }
    Item::ToolCall { id, name, input } => (
      Role::Assistant,
      json!({"type": "tool_use", "id": id.as_str(), "name": name, "input": input}),
    ),
    Item::Reasoning { opaque, .. } if !strip && opaque.wire() == WireApi::AnthropicMessages => {
      let block = serde_json::from_str(opaque.json())
        .map_err(|error| ModelError::Protocol(format!("a stored thinking block is not JSON: {error}")))?;
      (Role::Assistant, block)
    }
    Item::Reasoning { .. } => return Ok(None),
  };
  Ok(Some(block))
}
