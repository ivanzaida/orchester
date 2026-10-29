//! `<select>` drop-down lists, which a headless frame does not show (`docs/design/browser.md`, "What frames do not
//! show"): a press on one is found by a hit test and its options read for the window to draw; the person's choice is
//! set on the element with the events a page listens to.

use orchester_ipc::browser::{SelectId, SelectList};
use serde_json::{Value, json};

use crate::browser::{
  BrowserError,
  cdp::{Cdp, CdpError, SessionId},
};

/// Reads a single-choice `<select>` that the person may open, focusing it, as a [`SelectList`] named `select`; `null`
/// for any other element.
const READ_LIST: &str = "function(select) {
  if (this.tagName !== 'SELECT' || this.multiple || this.size > 1 || this.disabled) return null;
  this.focus();
  const box = this.getBoundingClientRect();
  return {
    select,
    options: Array.from(this.options, (option) => ({
      label: option.label, selected: option.selected, disabled: option.disabled,
    })),
    element: {
      x: Math.round(box.left), y: Math.round(box.top),
      width: Math.max(0, Math.round(box.width)), height: Math.max(0, Math.round(box.height)),
    },
  };
}";

/// Chooses option `index` of the `<select>` as the person's own choice would; `false` for no such enabled option.
const CHOOSE: &str = "function(index) {
  if (this.tagName !== 'SELECT' || index >= this.options.length || this.options[index].disabled) return false;
  this.selectedIndex = index;
  this.dispatchEvent(new Event('input', { bubbles: true }));
  this.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
}";

/// The `<select>` list at `x`, `y` of the tab of `session`, in CSS pixels; `None` when another element, or none, is
/// there.
pub(super) async fn at(cdp: &Cdp, session: &SessionId, x: i32, y: i32) -> Result<Option<SelectList>, BrowserError> {
  let hit = cdp
    .call(
      Some(session),
      "DOM.getNodeForLocation",
      json!({ "x": x, "y": y, "includeUserAgentShadowDOM": false }),
    )
    .await;
  let node = match hit {
    Ok(node) => node,
    // Nothing is there, or the page is between documents: the press goes to the page as it is.
    Err(CdpError::Refused { .. }) => return Ok(None),
    Err(error) => return Err(error.into()),
  };
  let Some(node) = node.get("backendNodeId").and_then(Value::as_u64) else {
    return Ok(None);
  };
  let read = call_on(cdp, session, node, READ_LIST, &[json!({ "value": node })]).await?;
  if read.is_null() {
    return Ok(None);
  }
  serde_json::from_value(read)
    .map(Some)
    .map_err(|error| BrowserError::Element(format!("the list's options cannot be read: {error}")))
}

/// Chooses option `index` of the list `select` in the tab of `session`.
pub(super) async fn choose(cdp: &Cdp, session: &SessionId, select: SelectId, index: u32) -> Result<(), BrowserError> {
  let chosen = call_on(cdp, session, select.get(), CHOOSE, &[json!({ "value": index })]).await?;
  if chosen == json!(true) {
    Ok(())
  } else {
    Err(BrowserError::Element(format!(
      "the list has no option {index} to choose"
    )))
  }
}

/// Calls `function` on the page's node `node` with `arguments` and returns its value.
async fn call_on(
  cdp: &Cdp,
  session: &SessionId,
  node: u64,
  function: &str,
  arguments: &[Value],
) -> Result<Value, BrowserError> {
  let resolved = cdp
    .call(Some(session), "DOM.resolveNode", json!({ "backendNodeId": node }))
    .await?;
  let object = resolved
    .pointer("/object/objectId")
    .and_then(Value::as_str)
    .ok_or_else(|| BrowserError::Element("the element is no longer in the page".to_owned()))?;
  let called = cdp
    .call(
      Some(session),
      "Runtime.callFunctionOn",
      json!({
        "objectId": object,
        "functionDeclaration": function,
        "arguments": arguments,
        "returnByValue": true,
      }),
    )
    .await?;
  if let Some(exception) = called.get("exceptionDetails") {
    return Err(BrowserError::Element(format!("the page threw: {exception}")));
  }
  Ok(called.pointer("/result/value").cloned().unwrap_or(Value::Null))
}
