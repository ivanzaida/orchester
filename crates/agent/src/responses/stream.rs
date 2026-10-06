//! One streamed Responses answer, put together from its events (`docs/architecture/native-agent.md`
//! §3.2). Text, reasoning summaries and call arguments are collected from their deltas as they stream;
//! `response.output_item.done` then gives each output item whole, and a reasoning item is kept exactly as
//! it arrived (with its `encrypted_content`), to be sent back. A response counts only once
//! `response.completed` or `response.incomplete` arrived; before that it is dropped and the request sent
//! again; a response a policy stopped (`response.failed` with a policy code) ends as a refusal. Call arguments are read
//! strictly: text that is not a JSON object, or that the stop cut off, is kept as `{"INVALID_JSON": "<raw>"}`.

use std::collections::BTreeMap;

use orchester_core::{
  connection::WireApi,
  native::{
    item::{Item, ProviderPayload, ToolCallId},
    model::{ModelError, StopReason, StreamEvent},
    tools::invalid_json_input,
  },
  run::TokenUsage,
};
use serde_json::Value;

use super::stream_error;
use crate::sse::SseEvent;

/// A complete response.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Streamed {
  pub items: Vec<Item>,
  pub stop: StopReason,
  pub usage: TokenUsage,
}

impl Streamed {
  /// The response stopped at its output limit with a tool call, which may be cut off.
  pub(super) fn cut_at_tool(&self) -> bool {
    self.stop == StopReason::MaxOutput && self.items.iter().any(|item| matches!(item, Item::ToolCall { .. }))
  }
}

/// One output item while it streams.
#[derive(Clone, Debug)]
enum Output {
  Message {
    text: String,
    refusal: String,
  },
  /// A reasoning item: its summary so far, and the item as `response.output_item.done` gave it.
  Reasoning {
    summary: String,
    whole: Option<Value>,
  },
  FunctionCall {
    call_id: String,
    name: String,
    arguments: String,
  },
}

/// How the response ended.
#[derive(Clone, Debug)]
struct End {
  stop: StopReason,
  usage: TokenUsage,
}

/// The events of one response so far.
#[derive(Debug, Default)]
pub(super) struct Accumulator {
  /// Output items by their index, each with whether it was completed.
  outputs: BTreeMap<u64, (Output, bool)>,
  end: Option<End>,
}

impl Accumulator {
  pub(super) fn new() -> Self {
    Self::default()
  }

  /// Takes in `event`; text, reasoning summaries and tool calls are reported to `events` as they
  /// arrive.
  pub(super) fn event(
    &mut self,
    event: &SseEvent,
    events: &mut (dyn FnMut(StreamEvent) + Send),
  ) -> Result<(), ModelError> {
    let data: Value = serde_json::from_str(&event.data)
      .map_err(|error| ModelError::Protocol(format!("an event is not JSON: {error}")))?;
    let delta = || data["delta"].as_str().unwrap_or_default();
    match data["type"].as_str().unwrap_or_default() {
      "response.output_item.added" => self.added(&data, events)?,
      "response.output_text.delta" => {
        if let Output::Message { text, .. } = self.output(&data)? {
          text.push_str(delta());
          events(StreamEvent::Text(delta().to_owned()));
        }
      }
      "response.refusal.delta" => {
        if let Output::Message { refusal, .. } = self.output(&data)? {
          refusal.push_str(delta());
        }
      }
      "response.reasoning_summary_text.delta" => {
        if let Output::Reasoning { summary, .. } = self.output(&data)? {
          summary.push_str(delta());
          events(StreamEvent::Reasoning(delta().to_owned()));
        }
      }
      "response.function_call_arguments.delta" => {
        if let Output::FunctionCall { arguments, .. } = self.output(&data)? {
          arguments.push_str(delta());
        }
      }
      "response.output_item.done" => self.done(&data)?,
      "response.completed" | "response.incomplete" => self.end = Some(end(&data["response"])?),
      "response.failed" => self.end = Some(failed(&data["response"])?),
      "error" => return Err(stream_error(&data)),
      // `response.created`, the `.done` events of parts, and event types added later.
      _ => {}
    }
    Ok(())
  }

  /// Whether the response ended.
  pub(super) const fn ended(&self) -> bool {
    self.end.is_some()
  }

  /// The response, once [`Self::ended`]. A completed response whose message refused stopped as a
  /// refusal; one that called tools stopped for them. A refusal runs no tool.
  pub(super) fn finish(self) -> Result<Streamed, ModelError> {
    let end = self
      .end
      .ok_or_else(|| ModelError::Protocol("the response did not end".to_owned()))?;
    let mut items = Vec::new();
    let mut refused = false;
    for (output, done) in self.outputs.into_values() {
      refused |= matches!(&output, Output::Message { refusal, .. } if !refusal.is_empty());
      items.extend(item(output, done)?);
    }
    let called = items.iter().any(|item| matches!(item, Item::ToolCall { .. }));
    let stop = match end.stop {
      StopReason::EndTurn if refused => StopReason::Refusal { category: None },
      StopReason::EndTurn if called => StopReason::ToolUse,
      other => other,
    };
    Ok(Streamed {
      items,
      stop,
      usage: end.usage,
    })
  }

  fn added(&mut self, data: &Value, events: &mut (dyn FnMut(StreamEvent) + Send)) -> Result<(), ModelError> {
    let item = &data["item"];
    let text = |name: &str| item[name].as_str().unwrap_or_default().to_owned();
    let output = match item["type"].as_str().unwrap_or_default() {
      "message" => Output::Message {
        text: String::new(),
        refusal: String::new(),
      },
      "reasoning" => Output::Reasoning {
        summary: String::new(),
        whole: None,
      },
      "function_call" => {
        events(StreamEvent::ToolCallStarted { name: text("name") });
        Output::FunctionCall {
          call_id: text("call_id"),
          name: text("name"),
          arguments: text("arguments"),
        }
      }
      other => return Err(ModelError::Protocol(format!("unknown output item type `{other}`"))),
    };
    self.outputs.insert(index(data)?, (output, false));
    Ok(())
  }

  /// The whole item: it replaces what its deltas gave.
  fn done(&mut self, data: &Value) -> Result<(), ModelError> {
    let at = index(data)?;
    let item = &data["item"];
    let Some((output, done)) = self.outputs.get_mut(&at) else {
      return Err(ModelError::Protocol(format!(
        "output item {at} ended but never started"
      )));
    };
    match output {
      Output::Message { text, refusal } => {
        let parts = item["content"].as_array().map_or(&[][..], Vec::as_slice);
        *text = joined(parts, "output_text", "text");
        *refusal = joined(parts, "refusal", "refusal");
      }
      Output::Reasoning { summary, whole } => {
        let parts = item["summary"].as_array().map_or(&[][..], Vec::as_slice);
        *summary = joined(parts, "summary_text", "text");
        *whole = Some(item.clone());
      }
      Output::FunctionCall { arguments, .. } => {
        if let Some(text) = item["arguments"].as_str() {
          text.clone_into(arguments);
        }
      }
    }
    // An item the output limit cut off ends `incomplete`.
    *done = item["status"].as_str().is_none_or(|status| status == "completed");
    Ok(())
  }

  fn output(&mut self, data: &Value) -> Result<&mut Output, ModelError> {
    let at = index(data)?;
    match self.outputs.get_mut(&at) {
      Some((output, _)) => Ok(output),
      None => Err(ModelError::Protocol(format!(
        "a delta for output item {at}, which never started"
      ))),
    }
  }
}

fn index(data: &Value) -> Result<u64, ModelError> {
  data["output_index"]
    .as_u64()
    .ok_or_else(|| ModelError::Protocol("an output item event has no output_index".to_owned()))
}

/// The `field` texts of the `kind` parts, joined by an empty line.
fn joined(parts: &[Value], kind: &str, field: &str) -> String {
  let texts: Vec<&str> = parts
    .iter()
    .filter(|part| part["type"] == kind)
    .filter_map(|part| part[field].as_str())
    .collect();
  texts.join("\n\n")
}

/// How the response in a `response.completed` or `response.incomplete` event ended.
fn end(response: &Value) -> Result<End, ModelError> {
  let stop = match response["status"].as_str().unwrap_or_default() {
    "completed" => StopReason::EndTurn,
    "incomplete" => match response["incomplete_details"]["reason"].as_str().unwrap_or_default() {
      "max_output_tokens" => StopReason::MaxOutput,
      "content_filter" => StopReason::Filtered,
      other => return Err(ModelError::Protocol(format!("unknown incomplete reason `{other}`"))),
    },
    other => return Err(ModelError::Protocol(format!("the response ended as `{other}`"))),
  };
  Ok(End {
    stop,
    usage: usage(&response["usage"]),
  })
}

/// A response's usage as Orchester counts it (§2.2): input includes the cached tokens, and output the
/// reasoning tokens.
fn usage(usage: &Value) -> TokenUsage {
  let count = |value: &Value| value.as_u64().unwrap_or(0);
  TokenUsage {
    input: count(&usage["input_tokens"]),
    cached_input: count(&usage["input_tokens_details"]["cached_tokens"]),
    output: count(&usage["output_tokens"]),
  }
}

/// A failed response: a refusal when a policy stopped it (§3.1: shown as a problem with its category,
/// no tool runs), else the error it reports.
fn failed(response: &Value) -> Result<End, ModelError> {
  let error = &response["error"];
  match error["code"].as_str() {
    Some(category @ ("bio_policy" | "misalignment_policy_violation")) => Ok(End {
      stop: StopReason::Refusal {
        category: Some(category.to_owned()),
      },
      usage: usage(&response["usage"]),
    }),
    _ => Err(stream_error(error)),
  }
}

/// The conversation item of an output; `done` tells whether it was completed or cut off. A message
/// that refused is kept as its refusal text; reasoning that never completed has nothing to send back.
fn item(output: Output, done: bool) -> Result<Option<Item>, ModelError> {
  Ok(match output {
    Output::Message { text, refusal } => {
      let text = if text.is_empty() { refusal } else { text };
      (!text.is_empty()).then_some(Item::Assistant { text })
    }
    Output::Reasoning { summary, whole } => match whole {
      Some(whole) if done => {
        let opaque = ProviderPayload::new(WireApi::OpenAiResponses, whole.to_string())
          .map_err(|error| ModelError::Protocol(error.to_string()))?;
        let summary = Some(summary).filter(|text| !text.trim().is_empty());
        Some(Item::Reasoning { summary, opaque })
      }
      _ => None,
    },
    Output::FunctionCall {
      call_id,
      name,
      arguments,
    } => {
      let id = ToolCallId::new(&call_id).map_err(|error| ModelError::Protocol(error.to_string()))?;
      Some(Item::ToolCall {
        id,
        name,
        input: tool_input(&arguments, done),
      })
    }
  })
}

/// A call's input: the arguments when they are a complete JSON object, else the raw text wrapped as
/// core answers invalid input.
fn tool_input(raw: &str, done: bool) -> Value {
  match serde_json::from_str::<Value>(raw) {
    Ok(input) if done && input.is_object() => input,
    _ => invalid_json_input(raw),
  }
}
