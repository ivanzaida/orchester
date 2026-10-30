//! The person's copy and cut (`docs/design/browser.md`, "What frames do not show"): the page's selection, which only
//! the page knows, is read in the page; the window puts it on the system clipboard. Paste is an input like any other.

use orchester_ipc::browser::MAX_CLIPBOARD_TEXT;
use serde_json::{Value, json};

use super::order::Turn;
use crate::browser::{
  BrowserError,
  cdp::{Cdp, SessionId},
};

/// The text selected in the element with the focus, inside shadow roots and same-origin frames, or else in its
/// document; `null`, and nothing deleted, past `limit` UTF-16 units. A cross-origin frame's selection is out of reach.
/// An input without a selection range of its own (`email`, `number`) has its selection read as the document's. A
/// password field gives nothing, as in any browser. `delete` does nothing where nothing can be edited.
const TAKE: &str = "function(cut, limit) {
  let owner = document;
  let active = owner.activeElement;
  while (active) {
    const frame = active.tagName === 'IFRAME' ? active.contentDocument : null;
    const inside = frame || active.shadowRoot;
    if (!inside || !inside.activeElement) break;
    if (frame) owner = frame;
    active = inside.activeElement;
  }
  const field = active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA') ? active : null;
  if (field && field.type === 'password') return '';
  const text = field && typeof field.selectionStart === 'number'
    ? field.value.substring(field.selectionStart, field.selectionEnd)
    : String(owner.getSelection() ?? '');
  if (text.length > limit) return null;
  if (cut && text) owner.execCommand('delete');
  return text;
}";

/// The text selected in the tab of `session`, holding `turn` until the request has gone out; deleted from an editable
/// element when `cut`.
pub(super) async fn take(cdp: &Cdp, session: &SessionId, cut: bool, turn: Turn) -> Result<String, BrowserError> {
  let expression = format!("({TAKE})({cut}, {MAX_CLIPBOARD_TEXT})");
  let answered = cdp.call(
    Some(session),
    "Runtime.evaluate",
    json!({ "expression": expression, "returnByValue": true }),
  );
  drop(turn);
  let answer = answered.await?;
  if let Some(exception) = answer.get("exceptionDetails") {
    return Err(BrowserError::Element(format!("the page threw: {exception}")));
  }
  match answer.pointer("/result/value") {
    Some(Value::String(text)) => Ok(text.clone()),
    Some(Value::Null) => Err(BrowserError::Element(format!(
      "the selection is longer than the {MAX_CLIPBOARD_TEXT} characters that can be copied"
    ))),
    _ => Err(BrowserError::Element("the selection cannot be read".to_owned())),
  }
}
