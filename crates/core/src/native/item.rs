//! The items of a native conversation (`docs/architecture/native-agent.md` §2.1, §6.2). A conversation
//! only grows: items are appended, never changed, and what a provider returned that Orchester does not
//! read (thinking signatures, encrypted reasoning) is kept verbatim so that a later request sends the
//! same bytes back.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::connection::WireApi;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ItemError {
  #[error("a tool call id must be 1–{max} characters without whitespace", max = ToolCallId::MAX_CHARS)]
  ToolCallId,
  #[error("a provider payload must be JSON: {0}")]
  Payload(String),
}

/// The id a model gave one of its tool calls; the call's result names it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ToolCallId(String);

impl ToolCallId {
  pub const MAX_CHARS: usize = 256;

  pub fn new(value: &str) -> Result<Self, ItemError> {
    let length = value.chars().count();
    if (1..=Self::MAX_CHARS).contains(&length) && !value.chars().any(|c| c.is_whitespace() || c.is_control()) {
      Ok(Self(value.to_owned()))
    } else {
      Err(ItemError::ToolCallId)
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for ToolCallId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for ToolCallId {
  type Error = ItemError;

  fn try_from(value: String) -> Result<Self, ItemError> {
    Self::new(&value)
  }
}

impl From<ToolCallId> for String {
  fn from(id: ToolCallId) -> Self {
    id.0
  }
}

/// Provider JSON kept as it arrived, in the wire format it arrived in. It is sent back unchanged and
/// never shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPayload {
  wire: WireApi,
  json: String,
}

impl ProviderPayload {
  pub fn new(wire: WireApi, json: String) -> Result<Self, ItemError> {
    serde_json::from_str::<serde::de::IgnoredAny>(&json).map_err(|error| ItemError::Payload(error.to_string()))?;
    Ok(Self { wire, json })
  }

  pub const fn wire(&self) -> WireApi {
    self.wire
  }

  pub fn json(&self) -> &str {
    &self.json
  }
}

/// Where a user item came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserOrigin {
  /// The step's prompt: the first item of the conversation.
  Task,
  /// What a resumed invocation starts with: the continue prompt, a revise note or messages.
  Resume,
  /// A person's answer that no pending `ask_question` call took as its result.
  Answer,
  /// Orchester's reminder to call `finish_step`.
  Reminder,
  /// The request for a summary before compaction, and the note after it.
  Compaction,
}

/// One entry of a native conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
  User {
    text: String,
    origin: UserOrigin,
  },
  Assistant {
    text: String,
  },
  /// Reasoning: the summary the provider gave, if any, and its payload, never shown raw.
  Reasoning {
    summary: Option<String>,
    opaque: ProviderPayload,
  },
  /// A tool call as the model wrote it; `name` may name no tool, and is then answered with an error.
  ToolCall {
    id: ToolCallId,
    name: String,
    input: serde_json::Value,
  },
  ToolResult {
    id: ToolCallId,
    content: String,
    is_error: bool,
  },
  /// A summary of what came before; it starts a new segment (§6.3).
  Compaction {
    summary: String,
  },
}

impl Item {
  pub fn user(text: impl Into<String>, origin: UserOrigin) -> Self {
    Self::User {
      text: text.into(),
      origin,
    }
  }

  pub fn tool_result(id: ToolCallId, content: impl Into<String>, is_error: bool) -> Self {
    Self::ToolResult {
      id,
      content: content.into(),
      is_error,
    }
  }

  /// The bytes of text the item adds to a request, for estimating its tokens.
  pub fn estimated_bytes(&self) -> usize {
    match self {
      Self::User { text, .. } | Self::Assistant { text } | Self::Compaction { summary: text } => text.len(),
      Self::Reasoning { summary, opaque } => summary.as_ref().map_or(0, String::len) + opaque.json().len(),
      Self::ToolCall { name, input, .. } => name.len() + input.to_string().len(),
      Self::ToolResult { content, .. } => content.len(),
    }
  }
}
