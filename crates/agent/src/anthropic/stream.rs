//! One streamed Messages response, put together from its events (`docs/architecture/native-agent.md`
//! §3.1): every content block exactly as it streamed, thinking signatures included, and the stop reason
//! and usage of `message_delta`. A response counts only once `message_stop` arrived; before that it is
//! dropped and the request sent again. Tool input streams eagerly, so it is read strictly here: input
//! that is not a JSON object, or that the stop cut off, is kept as `{"INVALID_JSON": "<raw>"}`.

use std::collections::BTreeMap;

use orchester_core::{
  connection::WireApi,
  native::{
    item::{Item, ProviderPayload, ToolCallId},
    model::{ModelError, StopReason, StreamEvent},
    tools::invalid_json_input,
  },
  resource::ModelId,
  run::TokenUsage,
};
use serde_json::{Value, json};

use super::stream_error;
use crate::sse::SseEvent;

/// A complete response.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Streamed {
  pub items: Vec<Item>,
  pub stop: StopReason,
  pub usage: TokenUsage,
  /// The model that answered, as the endpoint named it.
  pub model: Option<String>,
}

impl Streamed {
  /// The response stopped at its output limit with a tool call, which may be cut off.
  pub(super) fn cut_at_tool(&self) -> bool {
    self.stop == StopReason::MaxOutput && self.items.iter().any(|item| matches!(item, Item::ToolCall { .. }))
  }

  /// The model that answered; the requested one when the endpoint named none.
  pub(super) fn served_by(&self, requested: &ModelId) -> ModelId {
    self
      .model
      .as_deref()
      .and_then(|model| ModelId::new(model).ok())
      .unwrap_or_else(|| requested.clone())
  }
}

#[derive(Clone, Debug)]
enum Block {
  Text(String),
  Thinking {
    thinking: String,
    signature: String,
  },
  /// A block sent back as it arrived (`redacted_thinking`).
  Opaque(Value),
  ToolUse {
    id: String,
    name: String,
    start: Value,
    raw: String,
  },
}

/// The usage counts reported so far; later reports replace earlier ones.
#[derive(Clone, Copy, Debug, Default)]
struct Usage {
  input: u64,
  cache_creation: u64,
  cache_read: u64,
  output: u64,
}

impl Usage {
  fn update(&mut self, usage: &Value) {
    let set = |field: &mut u64, name: &str| {
      if let Some(value) = usage[name].as_u64() {
        *field = value;
      }
    };
    set(&mut self.input, "input_tokens");
    set(&mut self.cache_creation, "cache_creation_input_tokens");
    set(&mut self.cache_read, "cache_read_input_tokens");
    set(&mut self.output, "output_tokens");
  }

  /// Usage as Orchester counts it (§2.2): cache writes and reads are input, reads are cached input.
  const fn counted(self) -> TokenUsage {
    TokenUsage {
      input: self
        .input
        .saturating_add(self.cache_creation)
        .saturating_add(self.cache_read),
      cached_input: self.cache_read,
      output: self.output,
    }
  }
}

/// The events of one response so far.
#[derive(Debug, Default)]
pub(super) struct Accumulator {
  /// Blocks by their index, each with whether `content_block_stop` ended it.
  blocks: BTreeMap<u64, (Block, bool)>,
  stop: Option<StopReason>,
  usage: Usage,
  model: Option<String>,
  ended: bool,
}

impl Accumulator {
  pub(super) fn new() -> Self {
    Self::default()
  }

  /// Takes in `event`; text, thinking and tool calls are reported to `events` as they arrive.
  pub(super) fn event(
    &mut self,
    event: &SseEvent,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<(), ModelError> {
    let data: Value = serde_json::from_str(&event.data)
      .map_err(|error| ModelError::Protocol(format!("an event is not JSON: {error}")))?;
    match data["type"].as_str().unwrap_or_default() {
      "message_start" => {
        self.model = data["message"]["model"].as_str().map(str::to_owned);
        self.usage.update(&data["message"]["usage"]);
      }
      "content_block_start" => self.start(&data, events)?,
      "content_block_delta" => self.delta(&data, events)?,
      "content_block_stop" => {
        if let Some((_, done)) = self.blocks.get_mut(&index(&data)?) {
          *done = true;
        }
      }
      "message_delta" => {
        self.usage.update(&data["usage"]);
        if let Some(reason) = data["delta"]["stop_reason"].as_str() {
          self.stop = Some(stop_reason(reason, &data["delta"]["stop_details"])?);
        }
      }
      "message_stop" => self.ended = true,
      "error" => return Err(stream_error(&data["error"])),
      // `ping`, and event types added later, which the API says clients should skip.
      _ => {}
    }
    Ok(())
  }

  /// The usage reported so far.
  pub(super) const fn usage(&self) -> TokenUsage {
    self.usage.counted()
  }

  /// Whether `message_stop` arrived.
  pub(super) const fn ended(&self) -> bool {
    self.ended
  }

  /// The response, once [`Self::ended`].
  pub(super) fn finish(self) -> Result<Streamed, ModelError> {
    let stop = self
      .stop
      .ok_or_else(|| ModelError::Protocol("the response named no stop reason".to_owned()))?;
    let mut items = Vec::new();
    for (block, done) in self.blocks.into_values() {
      items.extend(item(block, done)?);
    }
    Ok(Streamed {
      items,
      stop,
      usage: self.usage.counted(),
      model: self.model,
    })
  }

  fn start(&mut self, data: &Value, events: &mut (dyn FnMut(StreamEvent) + Send)) -> Result<(), ModelError> {
    let block = &data["content_block"];
    let text = |name: &str| block[name].as_str().unwrap_or_default().to_owned();
    let started = match block["type"].as_str().unwrap_or_default() {
      "text" => Block::Text(text("text")),
      "thinking" => Block::Thinking {
        thinking: text("thinking"),
        signature: text("signature"),
      },
      "redacted_thinking" => Block::Opaque(block.clone()),
      "tool_use" => {
        events(StreamEvent::ToolCallStarted { name: text("name") });
        Block::ToolUse {
          id: text("id"),
          name: text("name"),
          start: block["input"].clone(),
          raw: String::new(),
        }
      }
      other => return Err(ModelError::Protocol(format!("unknown content block type `{other}`"))),
    };
    self.blocks.insert(index(data)?, (started, false));
    Ok(())
  }

  fn delta(&mut self, data: &Value, events: &mut (dyn FnMut(StreamEvent) + Send)) -> Result<(), ModelError> {
    let at = index(data)?;
    let Some((block, _)) = self.blocks.get_mut(&at) else {
      return Err(ModelError::Protocol(format!(
        "a delta for block {at}, which never started"
      )));
    };
    let delta = &data["delta"];
    let text = |name: &str| delta[name].as_str().unwrap_or_default();
    match (delta["type"].as_str().unwrap_or_default(), block) {
      ("text_delta", Block::Text(body)) => {
        body.push_str(text("text"));
        events(StreamEvent::Text(text("text").to_owned()));
      }
      ("thinking_delta", Block::Thinking { thinking, .. }) => {
        thinking.push_str(text("thinking"));
        events(StreamEvent::Reasoning(text("thinking").to_owned()));
      }
      ("signature_delta", Block::Thinking { signature, .. }) => signature.push_str(text("signature")),
      ("input_json_delta", Block::ToolUse { raw, .. }) => raw.push_str(text("partial_json")),
      (other, _) => {
        return Err(ModelError::Protocol(format!(
          "unexpected delta `{other}` for block {at}"
        )));
      }
    }
    Ok(())
  }
}

fn index(data: &Value) -> Result<u64, ModelError> {
  data["index"]
    .as_u64()
    .ok_or_else(|| ModelError::Protocol("a content block event has no index".to_owned()))
}

fn stop_reason(reason: &str, details: &Value) -> Result<StopReason, ModelError> {
  Ok(match reason {
    "end_turn" | "stop_sequence" => StopReason::EndTurn,
    "tool_use" => StopReason::ToolUse,
    "max_tokens" => StopReason::MaxOutput,
    "model_context_window_exceeded" => StopReason::ContextExceeded,
    "refusal" => StopReason::Refusal {
      category: details["category"].as_str().map(str::to_owned),
    },
    other => return Err(ModelError::Protocol(format!("unknown stop reason `{other}`"))),
  })
}

/// The conversation item of a block; `done` tells whether the block was ended or cut off.
fn item(block: Block, done: bool) -> Result<Option<Item>, ModelError> {
  let payload = |value: Value| {
    ProviderPayload::new(WireApi::AnthropicMessages, value.to_string())
      .map_err(|error| ModelError::Protocol(error.to_string()))
  };
  Ok(match block {
    Block::Text(text) if text.is_empty() => None,
    Block::Text(text) => Some(Item::Assistant { text }),
    Block::Thinking { thinking, signature } => {
      let opaque = payload(json!({"type": "thinking", "thinking": thinking, "signature": signature}))?;
      let summary = Some(thinking).filter(|text| !text.trim().is_empty());
      Some(Item::Reasoning { summary, opaque })
    }
    Block::Opaque(value) => Some(Item::Reasoning {
      summary: None,
      opaque: payload(value)?,
    }),
    Block::ToolUse { id, name, start, raw } => {
      let id = ToolCallId::new(&id).map_err(|error| ModelError::Protocol(error.to_string()))?;
      Some(Item::ToolCall {
        id,
        name,
        input: tool_input(start, &raw, done),
      })
    }
  })
}

/// A tool call's input: the streamed JSON when it is a complete object, else the raw text wrapped as
/// the provider asks for invalid input.
fn tool_input(start: Value, raw: &str, done: bool) -> Value {
  if raw.is_empty() && done && start.is_object() {
    return start;
  }
  match serde_json::from_str::<Value>(raw) {
    Ok(input) if done && input.is_object() => input,
    _ => invalid_json_input(raw),
  }
}
