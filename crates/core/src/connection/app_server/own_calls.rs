//! Calls of Orchester's tools the runner asked a person about itself. Codex runs Orchester's tools without asking (its
//! configuration approves them), so the runner asks before it runs a call that needs an approval; the call's end then
//! says a person was asked, as for a call Codex asked about. The runner's request and Codex's start of the call come
//! on different channels, so either may be seen first: a request whose call has not started yet is kept here until
//! it does.

use serde_json::Value;

/// The runner's requests about calls of Orchester's tools that have not started yet: the tool and its arguments.
#[derive(Default)]
pub(super) struct AskedOwnCalls(Vec<(String, Value)>);

impl AskedOwnCalls {
  pub(super) fn remember(&mut self, action: &str, arguments: &Value) {
    self.0.push((action.to_owned(), arguments.clone()));
  }

  /// Whether the runner asked about a call of `action` with `arguments`, forgetting that request.
  pub(super) fn take(&mut self, action: &str, arguments: &Value) -> bool {
    let found = self
      .0
      .iter()
      .position(|(asked, input)| asked == action && input == arguments);
    found.map(|at| self.0.remove(at)).is_some()
  }
}
