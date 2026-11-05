//! How full a Claude Code session's context is: what the latest message of the main conversation took in, against
//! the window a result's `modelUsage` reports for its model. Claude Code reports a window only in a turn's result, and
//! each turn of a resumed session is a CLI of its own: the window an earlier CLI reported for the same session and
//! model is carried into the next one's stream ([`ClaudeStream::carrying`]), so its readings before its own result
//! do not fall back to the table's estimate (ORC-704).

use serde_json::Value;

use super::{ClaudeStream, text};
use crate::connection::context_use::ContextUse;

impl ClaudeStream {
  /// The stream with `window`, the context window an earlier CLI of the session it resumes reported on the model this
  /// one runs on; `None` carries nothing. The caller tells that the session and the model are the same: a window
  /// reported for another model must never be carried.
  #[must_use]
  pub const fn carrying(mut self, window: Option<u64>) -> Self {
    self.carried = window;
    self
  }

  /// How full the session's context is after its latest message, against the window a result reported for its model
  /// (the only one reported, when the result names it otherwise); before any result did, the window carried from an
  /// earlier CLI of the session, and without one the table's estimate. `None` while no message of the main
  /// conversation reported its tokens.
  pub fn context(&self) -> Option<ContextUse> {
    let (used, model) = self.prompt.as_ref()?;
    let reported = self
      .windows
      .get(model)
      .or_else(|| match self.windows.len() {
        1 => self.windows.values().next(),
        _ => None,
      })
      .copied()
      .or(self.carried);
    Some(reported.map_or_else(
      || ContextUse::of_model(*used, model),
      |window| ContextUse::reported(*used, window),
    ))
  }

  /// The tokens a message of the main conversation took in; a subagent's messages fill a context of their own, and a
  /// message without tokens (the CLI's own, when it is not signed in) tells nothing.
  pub(super) fn note_prompt(&mut self, value: &Value, message: &Value) {
    if value.get("parent_tool_use_id").is_some_and(|parent| !parent.is_null()) {
      return;
    }
    let Some(used) = message.get("usage").and_then(prompt_tokens).filter(|used| *used > 0) else {
      return;
    };
    self.prompt = Some((used, text(message, "model").unwrap_or_default()));
  }

  /// The context windows a result's `modelUsage` reports, by model.
  pub(super) fn note_windows(&mut self, value: &Value) {
    let models = value.get("modelUsage").and_then(Value::as_object).into_iter().flatten();
    for (model, used) in models {
      if let Some(window) = used
        .get("contextWindow")
        .and_then(Value::as_u64)
        .filter(|window| *window > 0)
      {
        self.windows.insert(model.clone(), window);
      }
    }
  }
}

/// What a message's request took in, as a `usage` object reports it: input, cache writes and cache reads.
fn prompt_tokens(usage: &Value) -> Option<u64> {
  let count = |name| usage.get(name).and_then(Value::as_u64).unwrap_or(0);
  Some(
    usage
      .get("input_tokens")?
      .as_u64()?
      .saturating_add(count("cache_creation_input_tokens"))
      .saturating_add(count("cache_read_input_tokens")),
  )
}
