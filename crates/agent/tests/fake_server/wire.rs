//! The two wire formats the fake server speaks, and where they differ for its checks.

use serde_json::json;

/// The wire format the server speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
  /// Anthropic Messages at `/v1/messages`.
  Messages,
  /// OpenAI Responses at `/v1/responses`.
  Responses,
}

impl Wire {
  /// Where model requests go.
  pub(super) const fn path(self) -> &'static str {
    match self {
      Self::Messages => "/v1/messages",
      Self::Responses => "/v1/responses",
    }
  }

  /// The request fields that never change within a conversation.
  pub(super) const fn frozen(self) -> [&'static str; 2] {
    match self {
      Self::Messages => ["system", "tools"],
      Self::Responses => ["instructions", "tools"],
    }
  }

  /// The request field holding the history.
  pub(super) const fn history(self) -> &'static str {
    match self {
      Self::Messages => "messages",
      Self::Responses => "input",
    }
  }

  /// Where a stream event reports the cache read.
  pub(super) const fn cache_read(self) -> &'static str {
    match self {
      Self::Messages => "/message/usage/cache_read_input_tokens",
      Self::Responses => "/response/usage/input_tokens_details/cached_tokens",
    }
  }

  /// An error answer's body; Responses name `kind` as the error's type and code.
  pub(super) fn error_body(self, kind: &str, message: &str) -> String {
    let body = match self {
      Self::Messages => json!({"type": "error", "error": {"type": kind, "message": message}}),
      Self::Responses => json!({"error": {"type": kind, "code": kind, "message": message, "param": null}}),
    };
    body.to_string()
  }
}
