//! How full a Codex thread's context is, from the usage its responses report.

use serde_json::Value;

use super::usage;
use crate::connection::context_use::ContextUse;

/// How full the context is after the response `last` reports: its `totalTokens` (input and output, else their sum),
/// against `window`; `None` for a response that reports no tokens.
pub(super) fn context_use(last: &Value, window: Option<&Value>) -> Option<ContextUse> {
  let spent = usage(last);
  let used = last
    .get("totalTokens")
    .and_then(Value::as_u64)
    .unwrap_or_else(|| spent.input.saturating_add(spent.output));
  if used == 0 {
    return None;
  }
  Some(match window.and_then(Value::as_u64).filter(|window| *window > 0) {
    Some(window) => ContextUse::reported(used, window),
    None => ContextUse::unknown_window(used),
  })
}
